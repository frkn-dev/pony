use crate::api::requests::ApiRequests;
use crate::{
    agent::tasks::Tasks, http::debug::start_ws_server, utils::*, Agent, AgentSettings,
    HandlerActions, HandlerClient, Node, NodeStorage, State, StatsClient, Tag, User, UserStorage,
    XrayClient, XrayConfig, ZmqSubscriber,
};

use log::{debug, error, info};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};
use uuid::Uuid;

type AgentState = State<Node>;

pub async fn service(settings: AgentSettings) -> Result<(), Box<dyn std::error::Error>> {
    let debug = settings.debug.enabled;
    let mut tasks: Vec<JoinHandle<()>> = vec![];

    let xray_config = match XrayConfig::new(&settings.xray.xray_config_path) {
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

    let xray_api_endpoint = format!("http://{}", xray_config.api.listen.clone());

    let xray_stats_client = Arc::new(Mutex::new(StatsClient::new(&xray_api_endpoint).await?));
    let xray_handler_client = Arc::new(Mutex::new(HandlerClient::new(&xray_api_endpoint).await?));
    let subscriber = ZmqSubscriber::new(
        &settings.zmq.endpoint,
        &settings.node.uuid,
        &settings.node.env,
    );

    let node = Node::new(
        xray_config,
        settings
            .node
            .hostname
            .clone()
            .unwrap_or_else(|| "localhost".to_string()),
        settings
            .node
            .ipv4
            .unwrap_or_else(|| Ipv4Addr::new(127, 0, 0, 1)),
        settings.node.env.clone(),
        settings
            .node
            .default_interface
            .clone()
            .unwrap_or("eth0".to_string()),
        settings.node.uuid.clone(),
        settings.node.label.clone(),
    );

    let state: Arc<Mutex<AgentState>> = Arc::new(Mutex::new(State::with_node(node)));

    let agent = Arc::new(Agent::new(
        state.clone(),
        subscriber,
        xray_stats_client.clone(),
        xray_handler_client.clone(),
    ));

    if debug && !settings.agent.local {
        tokio::spawn(start_ws_server(
            state.clone(),
            settings
                .debug
                .web_server
                .unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
            settings.debug.web_port,
        ));
    }

    if settings.agent.metrics_enabled && !settings.agent.local {
        info!("Running metrics send task");
        let metrics_handle = tokio::spawn({
            let settings = settings.clone();
            let agent = agent.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.agent.metrics_interval)).await;
                    let _ = agent.send_metrics(settings.carbon.address.clone()).await;
                    debug!("Metrics send task tick");
                }
            }
        });
        tasks.push(metrics_handle);
    }

    if settings.agent.stat_enabled && !settings.agent.local {
        info!("Running Stat Task");
        let stats_task = tokio::spawn({
            let agent = agent.clone();
            async move {
                loop {
                    sleep(Duration::from_secs(settings.agent.stat_job_interval)).await;
                    let _ = agent.collect_stats().await;
                }
            }
        });
        tasks.push(stats_task);
    }

    if !settings.agent.local {
        info!("ZMQ listener starting...");

        let zmq_task = tokio::spawn({
            let agent = agent.clone();
            async move {
                if let Err(e) = agent.run_subscriber().await {
                    error!("ZMQ subscriber failed: {}", e);
                }
            }
        });
        tasks.push(zmq_task);

        let _ = {
            let settings = settings.clone();
            debug!("Register node task");
            if let Err(e) = agent
                .register_node(settings.api.endpoint.clone(), settings.api.token.clone())
                .await
            {
                panic!("Cannot register node {:?}", e);
            }
        };
    }

    let user_task = tokio::spawn({
        let username = Uuid::new_v4();
        let agent = agent.clone();
        async move {
            let _ = agent.xray_handler_client.create_all(username, None).await;

            let _ = {
                let mut state = agent.state.lock().await;
                let user = User::new(false, 1024, settings.node.env.clone(), None);
                let _ = state.add_or_update_user(username, user);
            };

            let state = agent.state.lock().await;
            if let Some(node) = state.nodes.get_node() {
                let vless_grpc_conn = vless_grpc_conn(
                    username,
                    node.ipv4,
                    node.inbounds
                        .get(&Tag::VlessGrpc)
                        .expect("VLESS gRPC inbound")
                        .clone(),
                    "🚀🚀🚀".to_string(),
                );
                let vless_xtls_conn = vless_xtls_conn(
                    username,
                    node.ipv4,
                    node.inbounds
                        .get(&Tag::VlessXtls)
                        .expect("VLESS XTLS inbound")
                        .clone(),
                    "🚀🚀🚀".to_string(),
                );
                let vmess_conn = vmess_tcp_conn(
                    username,
                    node.ipv4,
                    node.inbounds
                        .get(&Tag::Vmess)
                        .expect("VMESS inbound")
                        .clone(),
                );

                println!(
                    r#"
╔════════════════════════════════════╗
║ 🚀 Connection Details Ready 🚀     ║
╚════════════════════════════════════╝
"#
                );
                println!(
                    "🌐 VLESS gRPC ➜ {}\n",
                    vless_grpc_conn.expect("vless grpc conn")
                );
                println!(
                    "🔒 VLESS XTLS ➜ {}\n",
                    vless_xtls_conn.expect("vless xtls conn")
                );
                println!("✨ VMESS      ➜ {}\n", vmess_conn.expect("vmess conn"));
            } else {
                panic!("Node information is missing");
            }
        }
    });

    tasks.push(user_task);

    let _ = futures::future::join_all(tasks).await;
    Ok(())
}
