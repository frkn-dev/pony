use clap::Parser;
use fern::Dispatch;
use futures::future::join_all;
use log::{debug, error, info};
use std::{fmt, sync::Arc};
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};

use crate::{
    metrics::{
        bandwidth::bandwidth_metrics, cpuusage::cpu_metrics, loadavg::loadavg_metrics,
        memory::mem_metrics,
    },
    postgres::postgres_client,
    settings::{read_config, Settings},
    utils::{current_timestamp, human_readable_date, level_from_settings},
    xray_op::{config, stats::stats_task},
};

mod actions;
mod jobs;
mod message;
mod metrics;
mod node;
mod postgres;
mod settings;
mod state;
mod user;
mod utils;
mod xray_api;
mod xray_op;
mod zmq;

#[derive(Parser)]
#[command(version = "0.1.0", about = "Pony - control tool for Xray/Wireguard")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    println!("Config file {:?}", args.config);

    let mut settings: Settings = match read_config(&args.config) {
        Ok(settings) => settings,
        Err(err) => {
            println!("Wrong config file: {}", err);
            std::process::exit(1);
        }
    };

    // Logs handler init
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                record.level(),
                human_readable_date(current_timestamp()),
                record.target(),
                message
            ))
        })
        .level(level_from_settings(&settings.logging.level))
        .chain(std::io::stdout())
        .chain(fern::log_file(&settings.logging.file).unwrap())
        .apply()
        .unwrap();

    // Settings
    if let Err(e) = settings.validate() {
        error!("Error in settings: {}", e);
        std::process::exit(1);
    } else {
        info!(">>> Settings: {:?}", settings);
    }

    let debug = settings.app.debug;

    // For all tasks
    let mut tasks: Vec<JoinHandle<()>> = vec![];

    // Clients
    let pg_client = match postgres_client(settings.clone()).await {
        Ok(client) => client,
        Err(e) => panic!("PG not available, {}", e),
    };

    let xray_api_clients = match xray_op::client::create_clients(settings.clone()).await {
        Ok(clients) => clients,
        Err(e) => panic!("Can't create clients: {}", e),
    };

    // Xray-core Config Validation
    let xray_config = match config::read_xray_config(&settings.xray.xray_config_path) {
        Ok(config) => {
            debug!(
                "Xray Config: Successfully read Xray config file: {:?}",
                config
            );

            config.validate();
            config
        }
        Err(e) => {
            panic!("Xray Config:: Error reading JSON file: {}", e);
        }
    };

    // User State
    let state = match state::State::load_from_file_async(settings.app.file_state.clone()).await {
        Ok(state) => Arc::new(Mutex::new(state)),
        Err(e) => {
            debug!("State created from scratch, {}", e);
            let user_state = state::State::new(settings.clone(), xray_config.get_inbounds());
            if debug {
                let _ = user_state.save_to_file_async(&user_state.file_path).await;
            }
            Arc::new(Mutex::new(user_state))
        }
    };

    // Init and sync users

    //let users = jobs::users_db_request(pg_client).await;
    //
    //if let Ok(users) = users {
    //    let state = state.clone();
    //
    //    for user in users {
    //        sleep(Duration::from_millis(1000)).await;
    //
    //        if let Err(e) = jobs::init(
    //            state.clone(),
    //            settings.clone(),
    //            xray_api_clients.clone(),
    //            user,
    //            debug,
    //        )
    //        .await
    //        {
    //            eprintln!("Error processing user: {:?}", e);
    //        }
    //    }
    //
    //    let user_state = state.lock().await;
    //
    //    if let Err(e) = user_state.save_to_file_async("SYNC").await {
    //        eprintln!("Error saving state to file: {:?}", e);
    //    }
    //}
    //
    //let _ = {
    //    let user_state = state.lock().await;
    //
    //    // Например, вывести всех пользователей в state
    //    for (user_id, user) in &user_state.users {
    //        println!("User in state: {:?}, data: {:?}", user_id, user);
    //    }
    //};

    let users = jobs::users_db_request(pg_client).await;
    if let Ok(users) = users {
        let state = state.clone();

        let futures: Vec<_> = users
            .into_iter()
            .map(|user| {
                jobs::init(
                    state.clone(),
                    settings.clone(),
                    xray_api_clients.clone(),
                    user,
                    debug,
                )
            })
            .collect();

        let results = join_all(futures).await;

        for result in results {
            if let Err(e) = result {
                eprintln!("Error processing user: {:?}", e);
            }
        }
        let user_state = state.lock().await;
        let _ = user_state.save_to_file_async("SYNC").await;
    }

    // Statistics
    let stats_task = tokio::spawn(stats_task(xray_api_clients.clone(), state.clone(), debug));
    tasks.push(stats_task);

    let _ = {
        let user_state = state.clone();
        tasks.push(tokio::spawn(zmq::subscriber(
            xray_api_clients.clone(),
            settings.clone(),
            user_state,
            debug,
        )))
    };

    // Recurent Jobs
    let restore_trial_users_handle = tokio::spawn({
        debug!("Running restoring trial users job");
        let state = state.clone();
        let clients = xray_api_clients.clone();
        async move {
            loop {
                jobs::restore_trial_users(state.clone(), clients.clone(), debug).await;
                sleep(Duration::from_secs(60)).await;
            }
        }
    });
    tasks.push(restore_trial_users_handle);

    let block_trial_users_by_limit_handle = tokio::spawn({
        debug!("Running block trial users job");
        let state = state.clone();
        let clients = xray_api_clients.clone();
        async move {
            loop {
                sleep(Duration::from_secs(60)).await;
                jobs::block_trial_users_by_limit(state.clone(), clients.clone(), debug).await;
            }
        }
    });
    tasks.push(block_trial_users_by_limit_handle);

    // METRICS TASKS ========================
    let carbon_server = settings.carbon.address.clone();

    info!(">>> Running metric collector sends to {:?}", carbon_server);
    let metrics_tasks: Vec<JoinHandle<()>> = vec![
        tokio::spawn(bandwidth_metrics(carbon_server.clone(), settings.clone())),
        tokio::spawn(cpu_metrics(carbon_server.clone(), settings.clone())),
        tokio::spawn(loadavg_metrics(carbon_server.clone(), settings.clone())),
        tokio::spawn(mem_metrics(carbon_server.clone(), settings.clone())),
    ];

    for task in metrics_tasks {
        tasks.push(task);
    }
    //=======================================
    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}
