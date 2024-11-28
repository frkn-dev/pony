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
mod xray_api;
mod xray_op;

use crate::bandwidth::bandwidth_metrics;
use crate::config2::{read_config, Settings};
use crate::connections::connections_metric;
use crate::cpuusage::cpu_metrics;

use crate::loadavg::loadavg_metrics;
use crate::memory::mem_metrics;
use crate::utils::{current_timestamp, human_readable_date, level_from_settings};
use crate::web::not_found;

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

    //if settings.app.metrics_mode && settings.app.api_mode
    //    || !settings.app.metrics_mode && !settings.app.api_mode
    //{
    //    error!("Api and metrics mode enabled in the same time, choose mode");
    //    std::process::exit(1);
    //}

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

    if settings.app.xray_api_mode {
        let _settings_clone = settings.clone();

        let xray_api_client = xray_op::client::create_client(settings).await;

        let user_info = xray_op::vmess::UserInfo {
            in_tag: "VMess TCP".to_string(),
            level: 0,
            email: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            uuid: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        };

        match xray_op::vmess::remove_user(xray_api_client.clone(), user_info.clone()).await {
            Ok(()) => info!("User remove successfully {:?}", user_info.uuid),
            Err(e) => error!("User remove failed: {:?}", e),
        }

        match xray_op::vmess::add_user(xray_api_client, user_info.clone()).await {
            Ok(()) => info!("User add completed successfully {:?}", user_info.uuid),
            Err(e) => error!("User add operations failed: {:?}", e),
        }

        //let client_task = tokio::spawn(async move {
        //    match client_operations(settings_clone).await {
        //        Ok(_) => info!("Client operations completed successfully"),
        //        Err(e) => error!("Client operations failed: {:?}", e),
        //    }
        //});

        //grpcurl -plaintext -d '{"uuid": "550e8400-e29b-41d4-a716-446655440000"}' localhost:23456 xray.app.proxyman.command.HandlerService/GetUser
        //xray api statsquery --server=127.0.0.1:23456 -pattern "user_id:ebd8a62e-631f-49a8-979f-b1e0744891a3"

        //tasks.push(client_task);
    }

    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}
