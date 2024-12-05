use clap::Parser;
use fern::Dispatch;
use futures::future::join_all;
use log::{debug, info};
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

mod appconfig;
mod geoip;
mod metrics;
mod utils;
mod xray_api;
mod xray_op;
mod zmq;

use crate::appconfig::{read_config, Settings};
use crate::metrics::bandwidth::bandwidth_metrics;
use crate::metrics::connections::connections_metric;
use crate::metrics::cpuusage::cpu_metrics;
use crate::metrics::loadavg::loadavg_metrics;
use crate::metrics::memory::mem_metrics;
use crate::utils::{current_timestamp, human_readable_date, level_from_settings};
use crate::xray_op::stats::get_stats_task;
use crate::xray_op::users;
use crate::zmq::Tag;

#[derive(Parser)]
#[command(version = "0.0.23", about = "Pony - control tool for Xray/Wireguard")]
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
        info!(">>> Pony Version: 0.0.23");
    }

    let carbon_server = settings.carbon.address.clone();

    let mut tasks: Vec<JoinHandle<()>> = vec![];

    if settings.app.metrics_mode {
        info!(">>> Running metric collector sends to {:?}", carbon_server);
        let mut metrics_tasks: Vec<JoinHandle<()>> = vec![
            tokio::spawn(bandwidth_metrics(carbon_server.clone(), settings.clone())),
            tokio::spawn(cpu_metrics(carbon_server.clone(), settings.clone())),
            tokio::spawn(loadavg_metrics(carbon_server.clone(), settings.clone())),
            tokio::spawn(mem_metrics(carbon_server.clone(), settings.clone())),
        ];

        if settings.wg.enabled || settings.xray.enabled {
            metrics_tasks.push(tokio::spawn(connections_metric(
                carbon_server.clone(),
                settings.clone(),
            )));
        }

        for task in metrics_tasks {
            tasks.push(task);
        }
    }

    if settings.app.xray_api_mode {
        let xray_api_clients = match xray_op::client::create_clients(settings.clone()).await {
            Ok(clients) => clients,
            Err(e) => panic!("Can't create clients: {}", e),
        };

        let user_state =
            match users::UserState::load_from_file_async(settings.app.file_state.clone()).await {
                Ok(state) => {
                    debug!("State loaded from file");
                    Arc::new(Mutex::new(state))
                }
                Err(e) => {
                    debug!("State created from scratch, {}", e);
                    Arc::new(Mutex::new(users::UserState::new(
                        settings.app.file_state.clone(),
                    )))
                }
            };

        let sync_state_futures = vec![
            users::sync_state_to_xray_conf(
                user_state.clone(),
                xray_api_clients.clone(),
                Tag::Vless,
            ),
            users::sync_state_to_xray_conf(
                user_state.clone(),
                xray_api_clients.clone(),
                Tag::Vmess,
            ),
            users::sync_state_to_xray_conf(
                user_state.clone(),
                xray_api_clients.clone(),
                Tag::Shadowsocks,
            ),
        ];

        let _ = join_all(sync_state_futures).await;

        let tags = vec![Tag::Vmess, Tag::Vless, Tag::Shadowsocks];

        for tag in tags.clone() {
            let task = tokio::spawn(get_stats_task(
                xray_api_clients.clone(),
                user_state.clone(),
                tag.clone(),
            ));
            sleep(Duration::from_millis(100)).await;

            debug!("Task added, {tag:?}");
            tasks.push(task);
        }

        tasks.push(tokio::spawn(zmq::subscriber(
            xray_api_clients.clone(),
            settings.zmq.clone(),
            user_state.clone(),
        )));
    }

    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}
