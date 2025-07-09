use chrono::NaiveTime;
use fern::Dispatch;
use futures::future::join_all;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::time::Duration;

use pony::config::settings::ApiSettings;
use pony::config::settings::Settings;
use pony::http::debug;
use pony::metrics::Metrics;
use pony::utils::*;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::MemoryCache;
use pony::{PonyError, Result};

use crate::core::clickhouse::ChContext;
use crate::core::http::routes::Http;
use crate::core::postgres::run_shadow_sync;
use crate::core::postgres::PgContext;
use crate::core::sync::MemSync;
use crate::core::tasks::Tasks;
use crate::core::Api;
use crate::core::ApiState;

mod core;

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let config_path = &std::env::args()
        .nth(1)
        .expect("required config path as an argument");
    println!("Config file {:?}", config_path);

    let settings = ApiSettings::new(&config_path);

    settings.validate().expect("Wrong settings file");
    println!(">>> Settings: {:?}", settings.clone());

    // Logs handler init
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                record.level(),
                human_readable_date(current_timestamp() as u64),
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
        Ok(db) => db,
        Err(err) => {
            log::error!("Failed to init DB: {}", err);
            return Err(err.into());
        }
    };

    let ch = ChContext::new(&settings.clickhouse.address);
    let publisher = ZmqPublisher::new(&settings.zmq.endpoint).await;

    let mem: Arc<RwLock<ApiState>> = Arc::new(RwLock::new(MemoryCache::new()));
    let (tx, rx) = mpsc::channel(100);
    let mem_sync = MemSync::new(mem.clone(), tx);

    let api = Arc::new(Api::new(
        ch.clone(),
        publisher.clone(),
        mem_sync.clone(),
        settings.clone(),
    ));

    let _ = {
        match measure_time(db.user().all(), "db.user().all()".to_string()).await {
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

        match measure_time(db.node().all(), "db.node().all()".to_string()).await {
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

        match measure_time(db.conn().all(), "db.conn().all()".to_string()).await {
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

    if settings.api.metrics_enabled {
        log::info!("Running metrics send task");
        tokio::spawn({
            let settings = settings.clone();
            let api = api.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.api.metrics_interval)).await;
                    let _ = api.send_metrics(&settings.carbon.address).await;
                }
            }
        });
    }

    if settings.api.metrics_enabled {
        log::info!("Running HB metrics send task");
        tokio::spawn({
            let settings = settings.clone();
            let api = api.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.api.metrics_hb_interval)).await;
                    let _ = api.send_hb_metric(&settings.carbon.address).await;
                }
            }
        });
    }

    if debug {
        let token = Arc::new(settings.api.token);
        tokio::spawn(debug::start_ws_server(
            mem.clone(),
            settings
                .debug
                .web_server
                .unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
            settings.debug.web_port,
            token,
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
        let job_interval = Duration::from_secs(settings.api.collect_conn_stat_interval);

        async move {
            loop {
                tokio::time::sleep(job_interval).await;
                if let Err(e) = api.collect_conn_stat().await {
                    log::error!("collect_conn_stat task  failed: {:?}", e);
                }
            }
        }
    });

    let api_for_expire = api.clone();
    let _ = tokio::spawn({
        log::info!("enforce_all_trial_limits task started");
        let api = api_for_expire.clone();
        let job_interval = Duration::from_secs(settings.api.conn_limit_check_interval);

        async move {
            loop {
                if let Err(e) = api.enforce_xray_trial_limits().await {
                    log::error!("enforce_all_trial_limits task failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let api_for_restore = api.clone();
    log::info!("restore_trial_conns task started");
    let _ = tokio::spawn(run_daily(
        move || {
            let api = api_for_restore.clone();
            async move {
                if let Err(e) = api.restore_xray_trial_conns().await {
                    log::error!("Scheduled daily task failed: {:?}", e);
                }
            }
        },
        NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
    ));

    let _ = tokio::spawn(async move {
        run_shadow_sync(rx, db).await;
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
