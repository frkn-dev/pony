use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;
use warp::Filter;

use pony::http::filters as my_filters;
use pony::{ApiAccessConfig, NodeConfig};
use pony::{
    BaseConnection as Connection, ConnectionBaseOperations, Connections, MetricBuffer, Node,
    Publisher, Result, SnapshotManager, Subscriber,
};

use super::config::AuthServiceSettings;
use super::email::EmailStore;
use super::filters;
use super::handlers::{activate_key_handler, auth_handler};
use super::handlers::{tg_trial_handler, trial_handler};
use super::http::{ApiRequests, HttpClient};
use super::request;
use super::tasks::Tasks;

pub const PROTOS: [&str; 5] = [
    "VlessTcpReality",
    "VlessGrpcReality",
    "VlessXhttpReality",
    "Hysteria2",
    "Mtproto",
];

pub const DEFAULT_DAYS: i64 = 1;

pub struct AuthService<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<Connections<C>>>,
    pub metrics: Arc<MetricBuffer>,
    pub node: Node,
    pub subscriber: Subscriber,
    pub email_store: EmailStore,
    pub http_client: HttpClient,
    pub api: ApiAccessConfig,
    pub listen: Ipv4Addr,
    pub port: u16,
}

impl<C> AuthService<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static + std::fmt::Display,
{
    pub fn new(
        metrics: Arc<MetricBuffer>,
        node: Node,
        subscriber: Subscriber,
        email_store: EmailStore,
        http_client: HttpClient,
        api: ApiAccessConfig,
        listen: (Ipv4Addr, u16),
    ) -> Self {
        let memory = Arc::new(RwLock::new(Connections::default()));
        Self {
            memory,
            metrics,
            node,
            subscriber,
            email_store,
            http_client,
            api,
            listen: listen.0,
            port: listen.1,
        }
    }

    pub async fn start_auth_server(&self) {
        let health_check = warp::path("health-check").map(|| "Server OK");

        let cors = warp::cors()
            .allow_origin(self.email_store.web_host.as_str())
            .allow_credentials(true)
            .allow_methods(vec!["GET", "POST", "OPTIONS"])
            .allow_headers(vec!["Content-Type"])
            .max_age(86400)
            .build();

        let email_store = self.email_store.clone();
        let memory = self.memory.clone();
        let http_client = self.http_client.clone();
        let api = self.api.clone();

        let trial_route = warp::post()
            .and(warp::path("trial"))
            .and(warp::body::json::<request::Trial>())
            .and(filters::with_store(email_store.clone()))
            .and(my_filters::with_http_client(http_client.clone()))
            .and(filters::with_api_settings(api.clone()))
            .and_then(trial_handler)
            .with(&cors);

        let tg_trial_route = warp::post()
            .and(warp::path("tg-trial"))
            .and(warp::body::json::<request::TgTrial>())
            .and(my_filters::with_http_client(http_client.clone()))
            .and(filters::with_api_settings(api.clone()))
            .and_then(tg_trial_handler);

        let auth_route = warp::post()
            .and(warp::path("auth"))
            .and(warp::body::json::<request::Auth>())
            .and(warp::any().map(move || memory.clone()))
            .and_then(auth_handler);

        let activate_route = warp::post()
            .and(warp::path("activate"))
            .and(warp::body::json::<request::ActivateKey>())
            .and(my_filters::with_http_client(http_client))
            .and(filters::with_api_settings(api))
            .and_then(activate_key_handler)
            .with(&cors);

        let routes = health_check
            .or(auth_route)
            .or(trial_route)
            .or(tg_trial_route)
            .or(activate_route);

        warp::serve(routes)
            .run(SocketAddr::new(IpAddr::V4(self.listen), self.port))
            .await;
    }
}

pub async fn run(settings: AuthServiceSettings) -> Result<()> {
    let mut tasks: Vec<JoinHandle<()>> = vec![];
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let node_config = NodeConfig::from_raw(settings.node.clone());
    let node = Node::new(node_config?, None, None, None, None);

    let subscriber = Subscriber::new(
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
        .listen
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
                tracing::info!(
                    "Loaded {} auth connections from snapshot with ts {}",
                    count,
                    timestamp,
                );
                Some(timestamp)
            }
            Ok(None) => {
                tracing::error!("Snapshot file exists but couldn't be loaded");
                panic!("napshot file exists but couldn't be loaded")
            }
            Err(e) => {
                tracing::error!("Failed to load snapshot: {}", e);
                tracing::info!("Starting fresh due to snapshot load error");
                None
            }
        }
    } else {
        tracing::info!("No snapshot found, starting fresh");
        None
    };

    let snapshot_manager = snapshot_manager.clone();
    let snapshot_handle = tokio::spawn(async move {
        tracing::info!(
            "Running snapshot task, interval {}",
            settings.auth.snapshot_interval
        );

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            settings.auth.snapshot_interval,
        ));
        loop {
            interval.tick().await;
            if let Err(e) = snapshot_manager.create_snapshot().await {
                tracing::error!("Failed to create snapshot: {}", e);
            } else {
                let len = snapshot_manager.len().await;
                tracing::debug!("Auth snapshot saved successfully; {} connections", len);
            }
        }
    });

    tasks.push(snapshot_handle);

    {
        tracing::info!("ZMQ listener starting...");

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
                    tracing::warn!("Api not available {} retrying...", e);
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

    tracing::info!("Running metrics task");

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
    tracing::info!("Running flush metrics task");
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
                        tracing::info!("Task {i} completed successfully");
                    }

                    Err(e) => {
                        tracing::error!("Task {i} panicked: {:?}", e);
                        let _ = shutdown_tx.send(());
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        std::process::exit(1);
                    }
                }
            }
        } => {}
        _ = signal::ctrl_c() => {
            tracing::info!("🛑 Ctrl+C received. Shutting down...");
            let _ = shutdown_tx.send(());
            tokio::time::sleep(Duration::from_secs(5)).await;
            std::process::exit(0);
        }
    }
}
