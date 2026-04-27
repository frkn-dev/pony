use std::path::Path;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;

use tracing::{debug, error, info, warn};

use pony::{
    measure_time, BaseConnection as Connection, ConnectionBaseOperations, Connections,
    MetricBuffer, Node, Publisher, Result, SnapshotManager, Subscriber, Tag, WgApi, XrayClient,
    XrayHandlerClient, XrayStatsClient,
};

use pony::{H2Settings, HysteriaServerConfig, NodeConfig, WireguardSettings, XraySettings};

use super::config::AgentSettings;
use super::http::ApiRequests;
use super::snapshot::SnapshotRestore;
use super::tasks::Tasks;

pub struct Agent<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<Connections<C>>>,
    pub node: Node,
    pub metrics: Arc<MetricBuffer>,
    pub subscriber: Subscriber,
    pub xray_stats_client: Option<Arc<Mutex<XrayStatsClient>>>,
    pub xray_handler_client: Option<Arc<Mutex<XrayHandlerClient>>>,
    pub wg_client: Option<WgApi>,
}

impl<C> Agent<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static,
{
    pub fn new(
        node: Node,
        subscriber: Subscriber,
        metrics: Arc<MetricBuffer>,
        xray_stats_client: Option<Arc<Mutex<XrayStatsClient>>>,
        xray_handler_client: Option<Arc<Mutex<XrayHandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Self {
        let memory = Arc::new(RwLock::new(Connections::default()));
        Self {
            memory,
            node,
            metrics,
            subscriber,
            xray_stats_client,
            xray_handler_client,
            wg_client,
        }
    }
}

pub async fn run(settings: AgentSettings) -> Result<()> {
    let mut tasks: Vec<JoinHandle<()>> = vec![];
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Init Xray
    let (xray_config, xray_stats_client, xray_handler_client) = if settings.xray.enabled {
        let config = match XraySettings::new(&settings.xray.xray_config_path) {
            Ok(config) => {
                info!(
                    "Xray Config: Successfully read Xray config file: {:?}",
                    config
                );

                if let Err(e) = config.validate() {
                    panic!("Xray Config:: Error reading JSON file: {}", e);
                }
                config
            }
            Err(e) => {
                panic!("Xray Config:: Error reading JSON file: {}", e);
            }
        };

        let xray_api_endpoint = format!("http://{}", config.api.listen.clone());

        let stats_client = Arc::new(Mutex::new(XrayStatsClient::new(&xray_api_endpoint).await?));
        let handler_client = Arc::new(Mutex::new(
            XrayHandlerClient::new(&xray_api_endpoint).await?,
        ));

        (Some(config), Some(stats_client), Some(handler_client))
    } else {
        (None, None, None)
    };

    // Init Wireguard
    let (wg_client, wg_config) = if settings.wg.enabled {
        let config = WireguardSettings::new(&settings.wg);
        debug!("WG CONFIG {:?}", config);
        let client = match WgApi::new(&settings.wg.interface) {
            Ok(c) => c,
            Err(e) => panic!("Cannot create WG client: {}", e),
        };
        if let Err(e) = client.validate() {
            panic!("Cannot validate WG client: {}", e);
        }
        (Some(client), Some(config))
    } else {
        (None, None)
    };

    // Init Hysteria2
    let h2_config = if settings.h2.enabled {
        match HysteriaServerConfig::from_file(&settings.h2.path) {
            Ok(cfg) => {
                if let Err(e) = cfg.validate() {
                    error!("Hysteria2 config validation failed: {}", e);
                    panic!("Hysteria2 config: {}", e);
                } else {
                    match H2Settings::try_from(cfg) {
                        Ok(settings) => Some(settings),
                        Err(e) => {
                            error!("Hysteria2 validation error: {}", e);
                            panic!("Hysteria2 config: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to load Hysteria2 config: {}", e);
                panic!("Hysteria2 config: {}", e);
            }
        }
    } else {
        None
    };

    // Init Mtproto
    let mtproto_config = if settings.mtproto.enabled {
        Some(settings.mtproto.clone())
    } else {
        None
    };

    let node_config = NodeConfig::from_raw(settings.node.clone());
    let node = Node::new(
        node_config?,
        xray_config,
        wg_config.clone(),
        h2_config,
        mtproto_config,
    );

    let zmq_endpoint = settings.zmq.endpoint.clone();
    let subscriber = Subscriber::new(&zmq_endpoint, &node.uuid, &node.env.to_string());

    let metric_publisher = Publisher::connect(&settings.metrics.publisher).await;

    let metrics = MetricBuffer {
        batch: parking_lot::Mutex::new(Vec::new()),
        publisher: metric_publisher,
    };

    let agent = Arc::new(Agent::<Connection>::new(
        node.clone(),
        subscriber,
        Arc::new(metrics),
        xray_stats_client.clone(),
        xray_handler_client.clone(),
        wg_client.clone(),
    ));

    let snapshot_path = settings.agent.snapshot_path.clone();
    let snapshot_manager = SnapshotManager::new(snapshot_path, agent.memory.clone());

    let snapshot_timestamp = if Path::new(&snapshot_manager.snapshot_path).exists() {
        match snapshot_manager.load_snapshot().await {
            Ok(Some(timestamp)) => {
                if let Err(e) = snapshot_manager
                    .restore_connections(agent.xray_handler_client.clone(), wg_client)
                    .await
                {
                    error!("Couldn't restore connections from memory, {}", e);
                }
                let count = snapshot_manager.len().await;
                info!(
                    "Loaded {} connections from snapshot with ts  {}",
                    count, timestamp,
                );
                Some(timestamp)
            }
            Ok(None) => {
                warn!("Snapshot file exists but couldn't be loaded");
                None
            }
            Err(e) => {
                error!("Failed to load snapshot: {}", e);
                info!("Starting fresh due to snapshot load error");
                None
            }
        }
    } else {
        warn!("No snapshot found, starting fresh");
        None
    };

    tokio::spawn(async move {
        info!(
            "Running snapshot task, interval {}",
            settings.agent.snapshot_interval
        );

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            settings.agent.snapshot_interval,
        ));

        loop {
            interval.tick().await;
            if let Err(e) = measure_time(snapshot_manager.create_snapshot(), "Snapshot").await {
                error!("Failed to create snapshot: {}", e);
            } else {
                let count = snapshot_manager.len().await;
                debug!(
                    "Connections snapshot saved successfully; {} Connections",
                    count
                );
            }
        }
    });

    {
        info!("ZMQ listener starting...");
        let zmq_task = tokio::spawn({
            let agent = agent.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                tokio::select! {
                    _ = agent.run_subscriber() => {},
                    _ = shutdown.recv() => {},
                }
            }
        });
        tasks.push(zmq_task);

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        if !settings.agent.local {
            {
                let settings = settings.clone();
                let node = node.clone();

                loop {
                    match agent
                        .register_node(settings.api.endpoint.clone(), settings.api.token.clone())
                        .await
                    {
                        Ok(_) => {
                            let tags: Vec<_> = node
                                .inbounds
                                .keys()
                                .filter(|k| !matches!(k, Tag::Hysteria2)) // Hysteria2 uses external auth provider
                                .filter(|k| !matches!(k, Tag::Mtproto)) // Mtproto doesn't support auth provider
                                .collect();

                            for tag in tags {
                                agent
                                    .get_connections(
                                        settings.api.endpoint.clone(),
                                        settings.api.token.clone(),
                                        *tag,
                                        snapshot_timestamp,
                                    )
                                    .await?
                            }
                            break;
                        }
                        Err(e) => {
                            warn!("API unavailable, {} retrying... ", e);
                            sleep(Duration::from_secs(10)).await;
                        }
                    }
                }
            };
        }
    };

    info!("Running metrics task");

    let metrics_handle: JoinHandle<()> = tokio::spawn({
        let agent = agent.clone();
        let mut shutdown = shutdown_tx.subscribe();
        async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(settings.metrics.interval)) => {
                         agent.collect_metrics().await;

                    },
                    _ = shutdown.recv() => {
                        info!("🛑 Metrics task received shutdown");
                        break;
                    },
                }
            }
        }
    });

    info!("Running flush metrics task");
    let metrics_flush_handle: JoinHandle<()> = tokio::spawn({
        let agent = agent.clone();
        let mut shutdown = shutdown_tx.subscribe();
        async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(settings.metrics.interval+3)) => {
                         agent.metrics.flush_to_zmq().await;

                    },
                    _ = shutdown.recv() => {
                        info!("🛑 Metrics flush task received shutdown");
                        break;
                    },
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
                        info!("Task {i} completed successfully");
                    }

                    Err(e) => {
                        error!("Task {i} panicked: {:?}", e);
                        let _ = shutdown_tx.send(());
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        std::process::exit(1);
                    }
                }
            }
        } => {}
        _ = signal::ctrl_c() => {
            info!("🛑 Ctrl+C received. Shutting down...");
            let _ = shutdown_tx.send(());
            tokio::time::sleep(Duration::from_secs(5)).await;
            std::process::exit(0);
        }
    }
}
