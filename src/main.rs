use clap::Parser;
use fern::Dispatch;
use futures::future::join_all;
use log::info;
use log::{debug, error};
use metrics::metrics::MetricType;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};
use utils::measure_time;

use crate::{
    postgres::{postgres_client, users_db_request},
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

    // Settings
    let mut settings: Settings = match read_config(&args.config) {
        Ok(settings) => settings,
        Err(err) => {
            println!("Wrong config file: {}", err);
            std::process::exit(1);
        }
    };

    if let Err(e) = settings.validate() {
        eprintln!("Error in settings: {}", e);
        std::process::exit(1);
    } else {
        println!(">>> Settings: {:?}", settings.clone());
    }

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

    let debug = settings.app.debug;

    // For store all tasks
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
            info!(
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
    let state = {
        info!("Running User State Sync");
        let user_state = Arc::new(Mutex::new(state::State::new(
            settings.clone(),
            xray_config.get_inbounds(),
        )));
        // Init and sync users
        let users = measure_time(
            users_db_request(pg_client.clone(), settings.node.env.clone()),
            "db query".to_string(),
        )
        .await;
        if let Ok(users) = users {
            let futures: Vec<_> = users
                .into_iter()
                .map(|user| {
                    jobs::init_state(
                        user_state.clone(),
                        settings.clone(),
                        xray_api_clients.clone(),
                        user,
                    )
                })
                .collect();
            let results = measure_time(join_all(futures), "Init state".to_string()).await;

            for result in results {
                if let Err(e) = result {
                    error!("Error processing user: {:?}", e);
                }
            }
            if let Err(e) = jobs::register_node(user_state.clone(), settings.clone()).await {
                error!(
                    "Failed to register node on API {}: {}",
                    settings.api.endpoint_address, e,
                );
            } else {
                info!("Node {:?} is registered", settings.node.hostname);
            }
        }
        if debug {
            let user_state = user_state.lock().await;
            let _ = user_state.save_to_file_async(&user_state.file_path).await;
        }
        user_state
    };

    // ++ Recurent Jobs ++
    if settings.app.stat_enabled {
        // Statistics
        info!("Running Stat job");
        let stats_task = tokio::spawn(stats_task(xray_api_clients.clone(), state.clone()));
        tasks.push(stats_task);
    }

    if settings.app.trial_users_enabled {
        // Block trial users by traffic limit
        info!("Running trial users limit by traffic job");
        let block_trial_users_by_limit_handle = tokio::spawn({
            let state = state.clone();
            let clients = xray_api_clients.clone();
            async move {
                loop {
                    sleep(Duration::from_secs(settings.app.trial_jobs_timeout)).await;
                    jobs::block_trial_users_by_limit(state.clone(), clients.clone()).await;
                }
            }
        });
        tasks.push(block_trial_users_by_limit_handle);

        // Restore trial user
        info!("Running restoring trial users job");
        let restore_trial_users_handle = tokio::spawn({
            let state = state.clone();
            let clients = xray_api_clients.clone();
            async move {
                loop {
                    jobs::restore_trial_users(state.clone(), clients.clone()).await;
                    sleep(Duration::from_secs(settings.app.trial_jobs_timeout)).await;
                }
            }
        });
        tasks.push(restore_trial_users_handle);
    }

    // zeromq SUB messages listener
    let _ = {
        let settings = settings.clone();
        let user_state = state.clone();
        tasks.push(tokio::spawn(zmq::subscriber(
            xray_api_clients.clone(),
            settings.clone(),
            user_state,
        )))
    };

    // debug mode
    if debug {
        tokio::spawn(jobs::save_state_to_file_job(state.clone(), 10));
    }

    // METRICS TASKS
    if settings.app.metrics_enabled {
        info!("Running metrics send job");
        let metrics_handle = tokio::spawn({
            let state = state.clone();
            let settings = settings.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.app.metrics_timeout)).await;
                    let _ =
                        jobs::send_metrics_job::<MetricType>(state.clone(), settings.clone()).await;
                }
            }
        });
        tasks.push(metrics_handle);
    }

    // Run all tasks
    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}
