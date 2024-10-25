use ::clickhouse::Client;
use actix_web::middleware::Logger;
use actix_web::web::to;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use clap::Parser;
use fern::Dispatch;
use log::{error, info};
use std::fmt;
use std::sync::Arc;
use tokio::task::JoinHandle;

mod bandwidth;
mod clickhouse;
mod config2;
mod connections;
mod cpuusage;
mod geoip;
mod loadavg;
mod memory;
mod metrics;
mod utils;
mod web;
mod webhook;

use crate::bandwidth::bandwidth_metrics;
use crate::config2::{read_config, Settings};
use crate::connections::connections_metric;
use crate::cpuusage::cpu_metrics;
use crate::loadavg::loadavg_metrics;
use crate::memory::mem_metrics;
use crate::utils::{current_timestamp, human_readable_date, level_from_settings};
use crate::web::not_found;

#[derive(Parser)]
#[command(version = "0.0.1", about = "Pony - montiroing tool for Xray/Wireguard")]
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

    if settings.app.metrics_mode && settings.app.api_mode
        || !settings.app.metrics_mode && !settings.app.api_mode
    {
        error!("Api and metrics mode enabled in the same time, choose mode");
        std::process::exit(1);
    }

    let mut tasks: Vec<JoinHandle<()>> = vec![];

    if settings.app.metrics_mode {
        info!(
            ">>> Running metric collector pony sends to {:?}",
            carbon_server
        );
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

    if settings.app.api_mode {
        let ch_client = Arc::new(Client::default().with_url(&settings.clickhouse.address));
        let settings_arc = Arc::new(settings.clone());

        HttpServer::new(move || {
            let mut app = App::new()
                .app_data(Data::new(ch_client.clone()))
                .app_data(Data::new(settings_arc.clone()))
                .wrap(Logger::default())
                .service(web::hello)
                .service(web::status_ch)
                .service(web::status)
                .default_service(to(not_found));

            if settings.app.api_webhook_enabled {
                app = app.service(webhook::webhook_handler);
            }
            app
        })
        .bind((
            settings.app.api_bind_addr.as_str(),
            settings.app.api_bind_port,
        ))?
        .run()
        .await
        .expect("Run web server")
    }

    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}
