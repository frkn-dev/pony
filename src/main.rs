use fern::Dispatch;
use log::{error, info};
use std::fmt;
use tokio::task::JoinHandle;

mod bandwidth;
mod config2;
mod connections;
mod cpuusage;
mod geoip;
mod loadavg;
mod memory;
mod metrics;
mod utils;

use crate::bandwidth::bandwidth_metrics;
use crate::config2::{read_config, Settings};
use crate::connections::connections_metric;
use crate::cpuusage::cpu_metrics;
use crate::loadavg::loadavg_metrics;
use crate::memory::mem_metrics;
use crate::utils::{current_timestamp, human_readable_date, level_from_settings};

#[tokio::main]
async fn main() {
    let settings: Settings = match read_config("config.toml") {
        Ok(settings) => settings,
        Err(err) => {
            error!("Wrong config file: {}", err);
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

    info!(
        ">>> Running metric collector pony sends to {:?}",
        carbon_server
    );

    let mut tasks: Vec<JoinHandle<()>> = vec![
        tokio::spawn(bandwidth_metrics(carbon_server.clone(), settings.clone())),
        tokio::spawn(cpu_metrics(carbon_server.clone(), settings.clone())),
        tokio::spawn(loadavg_metrics(carbon_server.clone(), settings.clone())),
        tokio::spawn(mem_metrics(carbon_server.clone(), settings.clone())),
    ];

    if settings.wg.enabled || settings.xray.enabled {
        tasks.push(tokio::spawn(connections_metric(
            carbon_server.clone(),
            settings.clone(),
        )));
    }

    let _ = futures::future::try_join_all(tasks).await;
}
