use chrono::NaiveTime;
use clap::Parser;
use fern::Dispatch;
use futures::future::join_all;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::Duration;

use pony::clickhouse::ChContext;
use pony::config::settings::ApiSettings;
use pony::config::settings::Settings;
use pony::http::debug;
use pony::state::pg_run_shadow_sync;
use pony::state::PgContext;
use pony::state::State;
use pony::state::SyncState;
use pony::utils::*;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::ApiState;
use pony::{PonyError, Result};

use crate::core::http::routes::Http;
use crate::core::tasks::Tasks;
use crate::core::Api;

mod core;

#[derive(Parser)]
#[command(about = "Pony Api - control tool for Xray/Wireguard")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let args = Cli::parse();

    println!("Config file {:?}", args.config);

    let mut settings = ApiSettings::new(&args.config);

    if let Err(e) = settings.validate() {
        panic!("Wrong settings file {}", e);
    }
    println!(">>> Settings: {:?}", settings.clone());

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
        .apply()
        .unwrap();

    let debug = settings.debug.enabled;

    let db = match PgContext::init(&settings.pg).await {
        Ok(ctx) => ctx,
        Err(e) => panic!("DB error: {}", e),
    };

    let ch = ChContext::new(&settings.clickhouse.address);
    let publisher = ZmqPublisher::new(&settings.zmq.endpoint).await;

    let state: ApiState = State::new();
    let mem = Arc::new(Mutex::new(state));
    let (tx, rx) = mpsc::channel(100);
    let sync_state = SyncState::new(mem.clone(), tx);

    let api = Arc::new(Api::new(
        ch.clone(),
        publisher.clone(),
        sync_state.clone(),
        settings.clone(),
    ));

    let _ = {
        match measure_time(db.user().all(), "get all users()".to_string()).await {
            Ok(users) => {
                let futures: Vec<_> = users.into_iter().map(|user| api.add_user(user)).collect();

                if let Some(Err(e)) = measure_time(join_all(futures), "Add users".to_string())
                    .await
                    .into_iter()
                    .find(|r| r.is_err())
                {
                    log::error!("Error during node state initialization: {}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to fetch users from DB: {}", e);
                return Err(PonyError::Custom(e.to_string()));
            }
        }

        match measure_time(db.node().all(), "get all nodes()".to_string()).await {
            Ok(nodes) => {
                let futures: Vec<_> = nodes.into_iter().map(|node| api.add_node(node)).collect();

                if let Some(Err(e)) = measure_time(join_all(futures), "Add nodes".to_string())
                    .await
                    .into_iter()
                    .find(|r| r.is_err())
                {
                    log::error!("Error during node state initialization: {}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to fetch nodes from DB: {}", e);
                return Err(PonyError::Custom(e.to_string()));
            }
        }

        match measure_time(db.conn().all(), "get all connections from db".to_string()).await {
            Ok(conns) => {
                let futures: Vec<_> = conns.into_iter().map(|conn| api.add_conn(conn)).collect();

                if let Some(Err(e)) = measure_time(
                    join_all(futures),
                    "add all connections to state".to_string(),
                )
                .await
                .into_iter()
                .find(|r| r.is_err())
                {
                    log::error!("Error during conn state initialization: {}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to fetch conns from DB: {}", e);
                return Err(PonyError::Custom(e.to_string()));
            }
        }
    };

    if debug {
        tokio::spawn(debug::start_ws_server(
            mem.clone(),
            settings
                .debug
                .web_server
                .unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
            settings.debug.web_port,
        ));
    }

    let _ = tokio::spawn({
        log::info!("node_healthcheck task started");
        let job_interval = Duration::from_secs(settings.api.healthcheck_interval);
        let api = api.clone();

        async move {
            loop {
                if let Err(e) = api.node_healthcheck().await {
                    log::error!("node_healthcheck task  failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let _ = tokio::spawn({
        log::info!("collect_conn_stat task started");
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.conn_limit_check_interval);

        async move {
            loop {
                if let Err(e) = api.collect_conn_stat().await {
                    log::error!("collect_conn_stat task  failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let api_for_expire = api.clone();
    let _ = tokio::spawn({
        log::info!("check_limit_and_expire_conns task started");
        let api = api_for_expire.clone();
        let job_interval = Duration::from_secs(settings.api.conn_limit_check_interval);

        async move {
            loop {
                if let Err(e) = api.check_limit_and_expire_conns().await {
                    log::error!("check_limit_and_expire_conns task failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let api_for_restore = api.clone();
    let _ = tokio::spawn(run_daily(
        move || {
            let api = api_for_restore.clone();
            async move {
                if let Err(e) = api.restore_trial_conns().await {
                    log::error!("Scheduled daily task failed: {:?}", e);
                }
            }
        },
        NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
    ));

    let _ = tokio::spawn(async move {
        pg_run_shadow_sync(rx, db).await;
    });

    let api = api.clone();
    let api_handle = tokio::spawn(async move {
        if let Err(e) = api.run().await {
            eprintln!("API server exited with error: {}", e);
        }
    });

    let res: Result<()> = tokio::select! {
        _ = api_handle => {
            println!("API server finished");
            Ok(())
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C received, shutting down...");
            Ok(())
        }
    };
    res
}
