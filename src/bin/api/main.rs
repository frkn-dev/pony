use fern::Dispatch;
use pony::memory::node::Node;
use pony::memory::subscription::Subscription;
use pony::Connection;
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
    let metric_storage = Arc::new(MetricStorage::new(1000, 24 * 60 * 7, None));

    let api = Arc::new(Api::new(mem_sync.clone(), settings.clone(), metric_storage));

    measure_time(api.get_state_from_db(), "Init PG DB").await?;

    let api_clone = api.clone();
    tokio::spawn(async move {
        api_clone
            .periodic_db_sync(settings.api.db_sync_interval_sec)
            .await;
    });

    if settings.api.metrics_enabled {
        log::debug!("Running metrics task");
        // 1. Создаем сабскрайбер (BIND)
        let subscriber = Subscriber::new_bound("tcp://0.0.0.0:5555", vec![]);

        // 2. Создаем канал для Carbon (даже если пока пустой)
        let (carbon_tx, _carbon_rx) = tokio::sync::mpsc::unbounded_channel();

        // 3. Просто запускаем воркер!
        // Он сам внутри себя сделает tokio::spawn и начнет слушать.
        MetricWorker::start::<Node, Connection, Subscription>(
            api.metrics.clone(),
            subscriber,
            carbon_tx,
        )
        .await;

        log::info!("Metrics system initialized via MetricWorker");
    }

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
