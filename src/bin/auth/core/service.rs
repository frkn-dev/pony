use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::Duration;

use pony::config::settings::AuthServiceSettings;
use pony::config::settings::NodeConfig;
use pony::http::debug;
use pony::memory::node::Node;
use pony::MemoryCache;
use pony::Result;
use pony::SnapshotManager;
use pony::Subscriber as ZmqSubscriber;

use super::AuthService;
use crate::core::http::start_auth_server;
use crate::core::http::ApiRequests;
use crate::core::tasks::Tasks;
use crate::core::AuthServiceState;

pub async fn run(settings: AuthServiceSettings) -> Result<()> {
    let mut tasks: Vec<JoinHandle<()>> = vec![];
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let node_config = NodeConfig::from_raw(settings.node.clone());
    let node = Node::new(node_config?, None, None, None, None);

    let memory: Arc<RwLock<AuthServiceState>> =
        Arc::new(RwLock::new(MemoryCache::with_node(node.clone())));

    let subscriber = ZmqSubscriber::new(
        &settings.zmq.endpoint,
        &settings.node.uuid,
        &settings.node.env,
    );

    let auth = Arc::new(AuthService::new(memory.clone(), subscriber));

    let snapshot_manager =
        SnapshotManager::new(settings.clone().auth.snapshot_path, memory.clone());

    let snapshot_timestamp = if Path::new(&snapshot_manager.snapshot_path).exists() {
        match snapshot_manager.load_snapshot().await {
            Ok(Some(timestamp)) => {
                log::info!("Loaded connections snapshot from {}", timestamp);
                Some(timestamp)
            }
            Ok(None) => {
                log::warn!("Snapshot file exists but couldn't be loaded");
                None
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
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            settings.auth.snapshot_interval,
        ));
        loop {
            interval.tick().await;
            if let Err(e) = snapshot_manager.create_snapshot().await {
                log::error!("Failed to create snapshot: {}", e);
            } else {
                log::info!("Connections snapshot saved successfully");
            }
        }
    });

    let _ = {
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

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    };

    let _ = {
        let settings = settings.clone();
        if let Err(e) = auth
            .get_connections(
                settings.api.endpoint.clone(),
                settings.api.token.clone(),
                pony::Tag::Hysteria2,
                snapshot_timestamp,
            )
            .await
        {
            log::error!("-->>Cannot register auth service, \n {:?}", e);
        }
    };

    let _ = {
        let mut shutdown = shutdown_tx.subscribe();
        let memory = memory.clone();
        let addr = settings
            .auth
            .web_server
            .unwrap_or(Ipv4Addr::new(127, 0, 0, 1));
        let port = settings.auth.web_port;

        let auth_handle = tokio::spawn(async move {
            tokio::select! {
                _ = start_auth_server(memory, addr, port) => {},
                _ = shutdown.recv() => {},
            }
        });
        tasks.push(auth_handle);
    };

    let token = Arc::new(settings.api.token.clone());
    if settings.debug.enabled {
        log::debug!(
            "Running debug server: localhost:{}",
            settings.debug.web_port
        );
        let mut shutdown = shutdown_tx.subscribe();
        let memory = memory.clone();
        let addr = settings
            .debug
            .web_server
            .unwrap_or(Ipv4Addr::new(127, 0, 0, 1));
        let port = settings.debug.web_port;
        let token = token.clone();

        let debug_handle = tokio::spawn(async move {
            tokio::select! {
                _ = debug::start_ws_server(memory, addr, port, token) => {},
                _ = shutdown.recv() => {},
            }
        });
        tasks.push(debug_handle);
    }

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
