use fern::Dispatch;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

use pony::config::settings::ApiSettings;
use pony::config::settings::Settings;
use pony::metrics::storage::MetricStorage;
use pony::utils::*;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Result;
use pony::Subscriber;

use crate::core::http::routes::Http;
use crate::core::metrics::MetricWorker;
use crate::core::postgres::PgContext;
use crate::core::sync::MemSync;
use crate::core::tasks::Tasks;
use crate::core::Api;
use crate::core::ApiState;
use crate::core::Cache;

mod core;

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let config_path = &std::env::args()
        .nth(1)
        .expect("required config path as an argument");
    println!("Config file {:?}", config_path);

    let settings = ApiSettings::new(config_path);

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

    let db = match PgContext::init(&settings.pg).await {
        Ok(db) => db,
        Err(err) => {
            log::error!("Failed to init DB: {}", err);
            return Err(err);
        }
    };

    let publisher = ZmqPublisher::new(&settings.zmq.endpoint).await;

    let mem: Arc<RwLock<ApiState>> = Arc::new(RwLock::new(Cache::new()));
    let mem_sync = MemSync::new(mem.clone(), db.clone(), publisher.clone());
    let metric_storage = Arc::new(MetricStorage::new(
        settings.api.max_points,
        settings.api.retention_seconds,
    ));

    let api = Arc::new(Api::new(mem_sync.clone(), settings.clone(), metric_storage));

    measure_time(api.get_state_from_db(), "Init PG DB").await?;

    let api_clone = api.clone();
    tokio::spawn(async move {
        api_clone
            .periodic_db_sync(settings.api.db_sync_interval_sec)
            .await;
    });

    log::debug!("Running metrics reciever task");
    let subscriber = Subscriber::new_bound(&settings.metrics.reciever, settings.metrics.topic);

    MetricWorker::start(api.metrics.clone(), subscriber).await;

    log::info!("Metrics system initialized via MetricWorker");

    let metrics_storage = api.metrics.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let s = metrics_storage.clone();
            tokio::task::spawn_blocking(move || {
                log::info!("Starting MetricStorage GC...");
                s.perform_gc();
                log::info!("MetricStorage GC finished.");
            });
        }
    });

    tokio::spawn({
        let api = api.clone();

        let job_interval = Duration::from_secs(60);
        log::info!("cleanup_expired_connections task started");

        async move {
            api.cleanup_expired_connections(job_interval.as_secs())
                .await;
        }
    });

    tokio::spawn({
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.subscription_expire_interval);
        log::info!("cleanup_expired_subscriptions task started");

        async move {
            api.cleanup_expired_subscriptions(job_interval.as_secs())
                .await;
        }
    });

    tokio::spawn({
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.subscription_restore_interval);
        log::info!("restore_subscriptions task started");

        async move {
            api.restore_subscriptions(job_interval.as_secs()).await;
        }
    });

    let api = api.clone();
    let api_settings = settings.api.clone();
    let api_handle = tokio::spawn(async move {
        if let Err(e) = api.run(api_settings).await {
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
