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
    appconfig::{read_config, Settings},
    metrics::{
        bandwidth::bandwidth_metrics, cpuusage::cpu_metrics, loadavg::loadavg_metrics,
        memory::mem_metrics,
    },
    utils::{current_timestamp, human_readable_date, level_from_settings},
    xray_op::{config, stats::stats_task, user_state::UserState},
};

mod appconfig;
mod jobs;
mod message;
mod metrics;
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

    let settings: Settings = match read_config(&args.config) {
        Ok(settings) => settings,
        Err(err) => {
            println!("Wrong config file: {}", err);
            std::process::exit(1);
        }
    };

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

    if let Err(e) = settings.validate() {
        eprintln!("Error in settings: {}", e);
        std::process::exit(1);
    } else {
        info!(">>> Settings: {:?}", settings);
    }

    let carbon_server = settings.carbon.address.clone();

    let mut tasks: Vec<JoinHandle<()>> = vec![];

    if settings.app.metrics_mode {
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
    }

    if settings.app.xray_control_mode {
        let xray_api_clients = match xray_op::client::create_clients(settings.clone()).await {
            Ok(clients) => clients,
            Err(e) => panic!("Can't create clients: {}", e),
        };

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

        let user_state =
            match UserState::load_from_file_async(settings.app.file_state.clone()).await {
                Ok(state) => Arc::new(Mutex::new(state)),
                Err(e) => {
                    debug!("State created from scratch, {}", e);
                    let user_state =
                        UserState::new(settings.app.file_state.clone(), xray_config.get_inbounds());
                    let _ = user_state.save_to_file_async(&user_state.file_path).await;
                    Arc::new(Mutex::new(user_state))
                }
            };

        let inbounds = {
            let state = user_state.lock().await;

            state.node.inbounds.clone()
        };

        let sync_state_futures: Vec<_> = inbounds
            .keys()
            .map(|tag| jobs::sync_state(user_state.clone(), xray_api_clients.clone(), tag.clone()))
            .collect();

        let results = join_all(sync_state_futures).await;

        for result in results {
            if let Err(e) = result {
                error!("Error during sync: {:?}", e);
            }
        }

        let stats_task = tokio::spawn(stats_task(xray_api_clients.clone(), user_state.clone()));
        tasks.push(stats_task);

        let _ = {
            let user_state = user_state.clone();
            tasks.push(tokio::spawn(zmq::subscriber(
                xray_api_clients.clone(),
                settings.clone(),
                user_state,
            )))
        };

        let restore_trial_users_handle = tokio::spawn({
            debug!("Running restoring trial users job");
            let state = user_state.clone();
            let clients = xray_api_clients.clone();
            async move {
                loop {
                    jobs::restore_trial_users(state.clone(), clients.clone()).await;
                    sleep(Duration::from_secs(60)).await;
                }
            }
        });
        tasks.push(restore_trial_users_handle);

        let block_trial_users_by_limit_handle = tokio::spawn({
            debug!("Running block trial users job");
            let state = user_state.clone();
            let clients = xray_api_clients.clone();
            async move {
                loop {
                    sleep(Duration::from_secs(60)).await;
                    jobs::block_trial_users_by_limit(state.clone(), clients.clone()).await;
                }
            }
        });
        tasks.push(block_trial_users_by_limit_handle);
    }

    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}
