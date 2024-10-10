use config::{Config, File};
use fern::Dispatch;
use log::info;
use std::fmt;

mod bandwidth;
mod config2;
mod connections;
mod cpuusage;
mod loadavg;
mod memory;
mod metrics;
mod utils;
mod geoip;

use crate::bandwidth::bandwidth_metrics;
use crate::config2::Settings;
use crate::connections::connections_metric;
use crate::cpuusage::cpu_metrics;
use crate::loadavg::loadavg_metrics;
use crate::memory::mem_metrics;
use crate::utils::{current_timestamp, human_readable_date, level_from_settings};

#[tokio::main]
async fn main() {
    let settings = Config::builder()
        .add_source(File::with_name("config.toml"))
        .build()
        .unwrap();

    let settings: Settings = settings.try_deserialize().unwrap();

    println!("{:?}", settings);

    let carbon_server = settings.carbon.address;
    let app_settings = settings.app;

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

    info!(
        ">>> Running metric collector pony sends to {:?}",
        carbon_server
    );

    let bandwidth_task = tokio::spawn(bandwidth_metrics(
        carbon_server.clone(),
        app_settings.clone(),
    ));
    let cpu_task = tokio::spawn(cpu_metrics(carbon_server.clone(), app_settings.clone()));
    let load_avg_task = tokio::spawn(loadavg_metrics(carbon_server.clone(), app_settings.clone()));
    let mem_task = tokio::spawn(mem_metrics(carbon_server.clone(), app_settings.clone()));
    let connections_task = tokio::spawn(connections_metric(
        carbon_server.clone(),
        app_settings.clone(),
    ));

    let _ = tokio::try_join!(
        cpu_task,
        bandwidth_task,
        load_avg_task,
        mem_task,
        connections_task
    );
}
