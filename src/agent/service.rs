use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;

use crate::agent::tasks::Tasks;
use crate::api::http::debug::start_ws_server;
use crate::api::requests::ApiRequests;
use crate::config::settings::AgentSettings;
use crate::config::xray::Config as XrayConfig;
use crate::state::connection::Conn;
use crate::state::node::Node;
use crate::state::state::ConnStorage;
use crate::state::state::NodeStorage;
use crate::state::state::State;
use crate::state::tag::Tag;
use crate::utils::*;
use crate::xray_op::client::HandlerActions;
use crate::xray_op::client::HandlerClient;
use crate::xray_op::client::StatsClient;
use crate::xray_op::client::XrayClient;
use crate::zmq::subscriber::Subscriber as ZmqSubscriber;
use crate::Agent;

type AgentState = State<Node>;

pub async fn run(settings: AgentSettings) -> Result<(), Box<dyn std::error::Error>> {
    let debug = settings.debug.enabled;
    let mut tasks: Vec<JoinHandle<()>> = vec![];

    let xray_config = match XrayConfig::new(&settings.xray.xray_config_path) {
        Ok(config) => {
            log::info!(
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

    let node = Node::new(settings.node.clone(), xray_config);

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
        log::info!("Running metrics send task");
        let metrics_handle = tokio::spawn({
            let settings = settings.clone();
            let agent = agent.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.agent.metrics_interval)).await;
                    let _ = agent.send_metrics(settings.carbon.address.clone()).await;
                    log::debug!("Metrics send task tick");
                }
            }
        });
        tasks.push(metrics_handle);
    }

    if settings.agent.stat_enabled && !settings.agent.local {
        log::info!("Running Stat Task");
        let stats_task = tokio::spawn({
            let agent = Arc::new(agent.clone());
            async move {
                loop {
                    sleep(Duration::from_secs(settings.agent.stat_job_interval)).await;
                    let _ = <Arc<Agent<Node>> as Clone>::clone(&agent)
                        .collect_stats()
                        .await;
                }
            }
        });
        tasks.push(stats_task);
    }

    if !settings.agent.local {
        log::info!("ZMQ listener starting...");

        let zmq_task = tokio::spawn({
            let agent = agent.clone();
            async move {
                if let Err(e) = agent.run_subscriber().await {
                    log::error!("ZMQ subscriber failed: {}", e);
                }
            }
        });
        tasks.push(zmq_task);

        let _ = {
            let settings = settings.clone();
            log::debug!("Register node task");
            if let Err(e) = agent
                .register_node(settings.api.endpoint.clone(), settings.api.token.clone())
                .await
            {
                panic!("Cannot register node {:?}", e);
            }
        };
    }

    let conn_task = tokio::spawn({
        let conn_id = uuid::Uuid::new_v4();
        let agent = agent.clone();
        async move {
            let _ = agent.xray_handler_client.create_all(&conn_id, None).await;

            let _ = {
                let mut state = agent.state.lock().await;
                let conn = Conn::new(false, 1024, settings.node.env.clone(), None);
                let _ = state.connections.add_or_update(&conn_id, conn);
            };

            let state = agent.state.lock().await;
            if let Some(node) = state.nodes.get() {
                let vless_grpc_conn = vless_grpc_conn(
                    &conn_id,
                    node.address,
                    node.inbounds
                        .get(&Tag::VlessGrpc)
                        .expect("VLESS gRPC inbound")
                        .clone(),
                    "🚀🚀🚀".to_string(),
                );
                let vless_xtls_conn = vless_xtls_conn(
                    &conn_id,
                    node.address,
                    node.inbounds
                        .get(&Tag::VlessXtls)
                        .expect("VLESS XTLS inbound")
                        .clone(),
                    "🚀🚀🚀".to_string(),
                );
                let vmess_conn = vmess_tcp_conn(
                    &conn_id,
                    node.address,
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

    tasks.push(conn_task);

    let _ = futures::future::join_all(tasks).await;
    Ok(())
}
