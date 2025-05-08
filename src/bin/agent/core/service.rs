use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;

use pony::config::settings::AgentSettings;
use pony::config::xray::Config as XrayConfig;
use pony::http::debug;
use pony::state::AgentState;
use pony::state::ConnBase;
use pony::state::Node;
use pony::state::NodeStorage;
use pony::state::State;
use pony::utils::*;
use pony::xray_op::client::HandlerActions;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::xray_op::client::XrayClient;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::Result;

use super::tasks::Tasks;
use super::Agent;
use crate::core::http::ApiRequests;

pub async fn run(settings: AgentSettings) -> Result<()> {
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

    let state: Arc<Mutex<AgentState>> = Arc::new(Mutex::new(State::with_node(node.clone())));

    let agent = Arc::new(Agent::new(
        state.clone(),
        subscriber,
        xray_stats_client.clone(),
        xray_handler_client.clone(),
    ));

    if settings.agent.metrics_enabled && !settings.agent.local {
        log::info!("Running metrics send task");
        let metrics_handle = tokio::spawn({
            let settings = settings.clone();
            let agent = agent.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.agent.metrics_interval)).await;
                    let _ = agent.send_metrics(settings.carbon.address.clone()).await;
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
                    let _ = <Arc<Agent<Node, ConnBase>> as Clone>::clone(&agent)
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

        // uglyhuck for ZMQ
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let _ = {
            let settings = settings.clone();
            log::debug!("Register node task");
            if let Err(e) = agent
                .register_node(settings.api.endpoint.clone(), settings.api.token.clone())
                .await
            {
                panic!(
                    "-->>Cannot register node, use setting local mode for running no deps\n {:?}",
                    e
                );
            }
        };
    }

    let conn_task = tokio::spawn({
        let conn_id = uuid::Uuid::new_v4();
        let agent = agent.clone();
        let node = node.clone();
        async move {
            let _ = {
                let mut state = agent.state.lock().await;
                for (tag, _) in node.inbounds {
                    let conn = ConnBase::new(tag, None);
                    let _ = state.connections.insert(conn_id, conn.into());
                    let _ = xray_handler_client.create(&conn_id, tag, None).await;
                }
            };

            let state = agent.state.lock().await;
            if let Some(node) = state.nodes.get_self() {
                println!(
                    r#"
╔════════════════════════════════════╗
║ 🚀 Connection Details Ready 🚀     ║
╚════════════════════════════════════╝
"#
                );
                for (tag, inbound) in node.inbounds {
                    if let Ok(conn) = create_conn_link(
                        tag,
                        &node.uuid,
                        inbound.as_inbound_response(),
                        &node.label,
                        node.address,
                    ) {
                        println!("🔒  {tag}  ➜ {:?}\n", conn);
                    }
                }
            } else {
                panic!("Node information is missing");
            }
        }
    });

    let token = Arc::new(settings.api.token);
    if settings.debug.enabled {
        log::debug!(
            "Running debug server: localhost:{}",
            settings.debug.web_port
        );
        tokio::spawn(debug::start_ws_server(
            state.clone(),
            settings
                .debug
                .web_server
                .unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
            settings.debug.web_port,
            token,
        ));
    }

    tasks.push(conn_task);

    let _ = futures::future::join_all(tasks).await;
    Ok(())
}
