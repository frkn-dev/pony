use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;

use pony::config::settings::AuthServiceSettings;
use pony::config::settings::NodeConfig;
use pony::memory::node::Node;
use pony::metrics::storage::MetricBuffer;
use pony::BaseConnection as Connection;
use pony::Publisher;
use pony::Result;
use pony::SnapshotManager;
use pony::Subscriber as ZmqSubscriber;

use super::AuthService;
use crate::core::http::ApiRequests;
use crate::core::tasks::Tasks;
use crate::core::EmailStore;
use crate::core::HttpClient;

pub async fn run(settings: AuthServiceSettings) -> Result<()> {
    let mut tasks: Vec<JoinHandle<()>> = vec![];
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let node_config = NodeConfig::from_raw(settings.node.clone());
    let node = Node::new(node_config?, None, None, None, None);

    let subscriber = ZmqSubscriber::new(
        &settings.zmq.endpoint,
        &settings.node.uuid,
        &settings.node.env,
    );

    let email_store = EmailStore::new(
        settings.auth.email_file.clone(),
        settings.smtp.clone(),
        settings.auth.email_sign_token.clone(),
        settings.auth.web_host.clone(),
    );

    email_store.load_trials().await?;
    let http_client = HttpClient::new();

    let listen_addr = settings
        .auth
        .web_server
        .unwrap_or(Ipv4Addr::from_octets([127, 0, 0, 1]));

    let metric_publisher = Publisher::connect(&settings.metrics.publisher).await;

    let metrics = MetricBuffer {
        batch: parking_lot::Mutex::new(Vec::new()),
        publisher: metric_publisher,
    };

    let auth = Arc::new(AuthService::<Connection>::new(
        Arc::new(metrics),
        node,
        subscriber,
        email_store,
        http_client,
        settings.api.clone(),
        (listen_addr, settings.auth.web_port),
    ));

    let snapshot_manager =
        SnapshotManager::new(settings.clone().auth.snapshot_path, auth.memory.clone());

    let snapshot_timestamp = if Path::new(&snapshot_manager.snapshot_path).exists() {
        match snapshot_manager.load_snapshot().await {
            Ok(Some(timestamp)) => {
                let count = snapshot_manager.len().await;
                log::info!(
                    "Loaded {} auth connections from snapshot with ts {}",
                    count,
                    timestamp,
                );
                Some(timestamp)
            }
            Ok(None) => {
                log::error!("Snapshot file exists but couldn't be loaded");
                panic!("napshot file exists but couldn't be loaded")
            }
            Err(e) => {
                log::error!("Failed to load snapshot: {}", e);
                log::info!("Starting fresh due to snapshot load error");
                None
            }
        }
    } else {
        log::info!("No snapshot found, starting fresh");
        None
    };

    let snapshot_manager = snapshot_manager.clone();
    let snapshot_handle = tokio::spawn(async move {
        log::info!(
            "Running snapshot task, interval {}",
            settings.auth.snapshot_interval
        );

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            settings.auth.snapshot_interval,
        ));
        loop {
            interval.tick().await;
            if let Err(e) = snapshot_manager.create_snapshot().await {
                log::error!("Failed to create snapshot: {}", e);
            } else {
                let len = snapshot_manager.len().await;
                log::debug!("Auth snapshot saved successfully; {} connections", len);
            }
        }
    });

    tasks.push(snapshot_handle);

    {
        log::info!("ZMQ listener starting...");

        let zmq_task = tokio::spawn({
            let auth = auth.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                tokio::select! {
                    _ = auth.run_subscriber() => {},
                    _ = shutdown.recv() => {},
                }
            }
        });
        tasks.push(zmq_task);

        sleep(std::time::Duration::from_millis(500)).await;
    };

    {
        let settings = settings.clone();
        let auth = auth.clone();

        loop {
            let api_token = settings.api.token.clone();
            match auth
                .get_connections(
                    settings.api.endpoint.clone(),
                    api_token,
                    pony::Tag::Hysteria2,
                    snapshot_timestamp,
                )
                .await
            {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    log::warn!("Api not available {} retrying...", e);
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }
    };

    {
        let mut shutdown = shutdown_tx.subscribe();
        let auth = auth.clone();

        let auth_handle = tokio::spawn(async move {
            tokio::select! {
                _ = auth.start_auth_server() => {},
                _ = shutdown.recv() => {},
            }
        });
        tasks.push(auth_handle);
    };

    log::info!("Running metrics task");

    let auth_for_collect = auth.clone();
    let metrics_handle = tokio::spawn({
        let mut shutdown = shutdown_tx.subscribe();
        async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(settings.metrics.interval)) => {
                        auth_for_collect.collect_metrics().await;
                    },
                    _ = shutdown.recv() => break,
                }
            }
        }
    });

    let auth_for_flush = auth.clone();
    log::info!("Running flush metrics task");
    let metrics_flush_handle = tokio::spawn({
        let mut shutdown = shutdown_tx.subscribe();
        async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(settings.metrics.interval + 2)) => {
                        auth_for_flush.metrics.flush_to_zmq().await;
                    },
                    _ = shutdown.recv() => break,
                }
            }
        }
    });

    tasks.push(metrics_handle);
    tasks.push(metrics_flush_handle);

    wait_all_tasks_or_ctrlc(tasks, shutdown_tx).await;
    Ok(())
}

async fn wait_all_tasks_or_ctrlc(tasks: Vec<JoinHandle<()>>, shutdown_tx: broadcast::Sender<()>) {
    tokio::select! {
        _ = async {
            for (i, task) in tasks.into_iter().enumerate() {
                match task.await {
                    Ok(_) => {
                        log::info!("Task {i} completed successfully");
                    }

                    Err(e) => {
                        log::error!("Task {i} panicked: {:?}", e);
                        let _ = shutdown_tx.send(());
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        std::process::exit(1);
                    }
                }
            }
        } => {}
        _ = signal::ctrl_c() => {
            log::info!("🛑 Ctrl+C received. Shutting down...");
            let _ = shutdown_tx.send(());
            tokio::time::sleep(Duration::from_secs(5)).await;
            std::process::exit(0);
        }
    }
}
