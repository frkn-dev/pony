use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;

use pony::config::settings::AgentSettings;
use pony::config::wireguard::WireguardSettings;
use pony::config::xray::Config as XrayConfig;
use pony::http::debug;
use pony::metrics::Metrics;
use pony::state::connection::wireguard::Param as WgParam;

use pony::state::node::Node;
use pony::utils::*;
use pony::wireguard_op::WgApi;
use pony::xray_op::client::HandlerActions;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::xray_op::client::XrayClient;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::Base as Connection;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::Proto;
use pony::Result;
use pony::State;
use pony::Tag;

use super::tasks::Tasks;
use super::Agent;
use crate::core::http::ApiRequests;
use crate::core::AgentState;

pub async fn run(settings: AgentSettings) -> Result<()> {
    let mut tasks: Vec<JoinHandle<()>> = vec![];
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Init Xray
    let (xray_config, xray_stats_client, xray_handler_client) = if settings.xray.enabled {
        let config = match XrayConfig::new(&settings.xray.xray_config_path) {
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

        let xray_api_endpoint = format!("http://{}", config.api.listen.clone());

        let stats_client = Arc::new(Mutex::new(StatsClient::new(&xray_api_endpoint).await?));
        let handler_client = Arc::new(Mutex::new(HandlerClient::new(&xray_api_endpoint).await?));

        (Some(config), Some(stats_client), Some(handler_client))
    } else {
        (None, None, None)
    };

    // Init Wireguard
    let (wg_client, wg_config) = if settings.wg.enabled {
        let config = WireguardSettings::new(&settings.wg);
        log::debug!("WG CONFIG {:?}", config);
        if let Ok(client) = WgApi::new(&settings.wg.interface) {
            (Some(client), Some(config))
        } else {
            panic!("Cannot create WG client");
        }
    } else {
        (None, None)
    };

    let subscriber = ZmqSubscriber::new(
        &settings.zmq.endpoint,
        &settings.node.uuid,
        &settings.node.env,
    );

    let node = Node::new(settings.node.clone(), xray_config, wg_config.clone());

    let state: Arc<Mutex<AgentState>> = Arc::new(Mutex::new(State::with_node(node.clone())));

    let agent = Arc::new(Agent::new(
        state.clone(),
        subscriber,
        xray_stats_client.clone(),
        xray_handler_client.clone(),
        wg_client.clone(),
    ));

    if settings.agent.metrics_enabled {
        log::info!("Running metrics send task");
        let metrics_handle = tokio::spawn({
            let settings = settings.clone();
            let agent = agent.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        _ = sleep(Duration::from_secs(settings.agent.metrics_interval)) => {
                            let _ = agent.send_metrics(settings.carbon.address.clone()).await;

                        },
                        _ = shutdown.recv() => break,
                    }
                }
            }
        });
        tasks.push(metrics_handle);
    }

    if settings.agent.metrics_enabled {
        log::info!("Running HB metrics send task");
        let metrics_handle = tokio::spawn({
            let settings = settings.clone();
            let agent = agent.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        _ = sleep(Duration::from_secs(settings.agent.metrics_hb_interval)) => {
                            let _ = agent.send_hb_metric(settings.carbon.address.clone()).await;
                        },
                        _ = shutdown.recv() => break,
                    }
                }
            }
        });
        tasks.push(metrics_handle);
    }

    if settings.agent.stat_enabled && !settings.agent.local {
        log::info!("Running Stat Task");
        let stats_task = tokio::spawn({
            let agent = Arc::new(agent.clone());
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        _ = sleep(Duration::from_secs(settings.agent.stat_job_interval)) => {
                            let _ = <Arc<Agent<Node, Connection>> as Clone>::clone(&agent)
                                .collect_stats()
                                .await;
                        },
                        _ = shutdown.recv() => break,
                    }
                }
            }
        });
        tasks.push(stats_task);
    }

    if settings.agent.stat_enabled {
        log::info!("Running WG Stat Task");
        let wg_stats_task = tokio::spawn({
            let agent = Arc::new(agent.clone());
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        _ = sleep(Duration::from_secs(settings.agent.stat_job_interval)) => {
                            let _ = <Arc<Agent<Node, Connection>> as Clone>::clone(&agent)
                                .collect_wireguard_stats()
                                .await;
                        },
                        _ = shutdown.recv() => break,
                    }
                }
            }
        });
        tasks.push(wg_stats_task);
    }

    let _ = {
        log::info!("ZMQ listener starting...");

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
    };

    if settings.wg.enabled {
        let conn_task = tokio::spawn({
            let agent = agent.clone();
            let node = node.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                tokio::select! {
                    _ = async {
                        let mut state = agent.state.lock().await;
                        for (tag, _inbound) in &node.inbounds {
                            if tag.is_wireguard() {
                                let conn_id = uuid::Uuid::new_v4();

                                if let Some(wg_api) = &agent.wg_client {
                                    if let Some(ref wg_config) = wg_config {
                                    let next = match wg_api.next_available_ip_mask(&wg_config.network, &wg_config.address) {
                                        Ok(ip) => ip,
                                        Err(e) => {
                                            log::error!("Failed to get next available WireGuard IP: {}", e);
                                            return;
                                        }
                                    };
                                    let wg_params = WgParam::new(next.clone());
                                    let proto = Proto::new_wg(&wg_params, &node.uuid);
                                    let conn = Connection::new(proto);
                                    let _ = state.connections.insert(conn_id, conn.clone().into());

                                    if let Err(e) = wg_api.create(&wg_params.keys.pubkey, next) {
                                        log::error!(
                                            "Failed to register WireGuard peer (tag: {} {}): {}",
                                            tag,
                                            wg_params.keys.pubkey,
                                            e
                                        );
                                    }


                                    if let Some(inbound) = node.inbounds.get(&Tag::Wireguard) {
                                        log::debug!("Inbound {:?}", inbound);

                                        if let Some(wg) = conn.get_wireguard() {

                                                if let Ok(conn) = wireguard_conn(
                                                    &conn_id,
                                                    &node.address,
                                                    inbound.as_inbound_response(),
                                                    &node.label,
                                                    &wg.keys.privkey,
                                                    &wg.address,
                                                ) {
                                                    println!("\u{1f512}  {tag}  \u{279e}\n\n {}\n", conn);
                                                }

                                        }
                                    }
                                }

                                } else {
                                    log::warn!("WG API client is not available");
                                }
                            }
                        }
                    } => {},
                    _ = shutdown.recv() => {},
                }
            }
        });

        tasks.push(conn_task);
    }

    if settings.xray.enabled {
        if let Some(xray_handler_client) = xray_handler_client {
            let conn_task = tokio::spawn({
                let conn_id = uuid::Uuid::new_v4();
                let agent = agent.clone();
                let node = node.clone();
                let mut shutdown = shutdown_tx.subscribe();
                async move {
                    tokio::select! {
                        _ = async {
                            let _ = {
                                let mut state = agent.state.lock().await;
                                for (tag, _) in node.inbounds {
                                    let proto = Proto::new_xray(&tag);
                                    let conn = Connection::new(proto);
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
                        } => {},
                        _ = shutdown.recv() => {},
                    }
                }
            });
            tasks.push(conn_task);
        }
    }

    let token = Arc::new(settings.api.token);
    if settings.debug.enabled {
        log::debug!(
            "Running debug server: localhost:{}",
            settings.debug.web_port
        );
        let mut shutdown = shutdown_tx.subscribe();
        let state = state.clone();
        let addr = settings
            .debug
            .web_server
            .unwrap_or(Ipv4Addr::new(127, 0, 0, 1));
        let port = settings.debug.web_port;
        let token = token.clone();

        let debug_handle = tokio::spawn(async move {
            tokio::select! {
                _ = debug::start_ws_server(state, addr, port, token) => {},
                _ = shutdown.recv() => {},
            }
        });
        tasks.push(debug_handle);
    }

    tokio::select! {
        _ = futures::future::join_all(tasks) => {},
        _ = signal::ctrl_c() => {
            log::info!("🛑 Ctrl+C received. Shutting down...");
            let _ = shutdown_tx.send(());

            tokio::time::sleep(Duration::from_secs(5)).await;
            log::warn!("⏱ Timeout waiting for graceful shutdown. Forcing exit.");
            std::process::exit(0);
        }
    }
    Ok(())
}
