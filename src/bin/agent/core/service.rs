use pony::config::settings::NodeConfig;
use qrcode::render::unicode;
use qrcode::QrCode;
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
use pony::http::requests::NodeType;
use pony::memory::connection::wireguard::Param as WgParam;
use pony::memory::node::Node;
use pony::metrics::Metrics;
use pony::utils::*;
use pony::wireguard_op::wireguard_conn;
use pony::wireguard_op::WgApi;
use pony::xray_op::client::HandlerActions;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::xray_op::client::XrayClient;
use pony::BaseConnection as Connection;
use pony::ConnectionBaseOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::Proto;
use pony::Result;
use pony::Subscriber as ZmqSubscriber;
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

    let subscriber = ZmqSubscriber::new(
        &settings.zmq.endpoint,
        &settings.node.uuid,
        &settings.node.env,
    );

    let node_config = NodeConfig::from_raw(settings.node.clone());
    let node = Node::new(node_config?, xray_config, wg_config.clone());

    let memory: Arc<Mutex<AgentState>> = Arc::new(Mutex::new(MemoryCache::with_node(node.clone())));

    let agent = Arc::new(Agent::new(
        memory.clone(),
        subscriber,
        xray_stats_client.clone(),
        xray_handler_client.clone(),
        wg_client.clone(),
    ));

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
                                let mut memory = agent.memory.lock().await;
                                for (tag, _) in node.inbounds {
                                    let proto = Proto::new_xray(&tag);
                                    let conn = Connection::new(proto);
                                    let _ = memory.connections.insert(conn_id, conn.into());
                                    let _ = xray_handler_client.create(&conn_id, tag, None).await;
                                }
                            };

                            let mem = agent.memory.lock().await;
                            if let Some(node) = mem.nodes.get_self() {
                                println!(
                                    r#"
╔════════════════════════════════════╗
║       Xray Connection Details      ║ 
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
                                        println!("->>  {tag}  ➜ {:?}\n", conn);
                                        let qrcode = QrCode::new(conn).unwrap();

                                        let image = qrcode
                                                .render::<unicode::Dense1x2>()
                                                .quiet_zone(true)
                                                .min_dimensions(1, 1)
                                                .build();


                                        println!("{}\n", image);
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

    if settings.wg.enabled {
        let conn_task = tokio::spawn({
            let agent = agent.clone();
            let settings = settings.clone();
            let node = node.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                tokio::select! {
                    _ = async {
                        let mut mem = agent.memory.lock().await;
                        for (tag, _inbound) in &node.inbounds {
                            if tag.is_wireguard() {
                                let conn_id = uuid::Uuid::new_v4();

                                if let Some(wg_api) = &agent.wg_client {
                                    if let Some(ref wg_config) = wg_config {
                                    let next = match wg_api.next_available_ip_mask(&wg_config.network, &wg_config.address) {
                                        Ok(ip) => ip,
                                        Err(e) => {
                                            log::error!("Failed to get next available WireGuard IP: {}; Check your Wireguard interface {}",
                                                e, settings.wg.interface);
                                            return Err(format!( "Failed to get next available WireGuard IP: {}; Check your Wireguard interface {}",
                                                e, settings.wg.interface));
                                        }
                                    };
                                    let wg_params = WgParam::new(next.clone());
                                    let proto = Proto::new_wg(&wg_params, &node.uuid);
                                    let conn = Connection::new(proto);
                                    let _ = mem.connections.insert(conn_id, conn.clone().into());

                                    if let Err(e) = wg_api.create(&wg_params.keys.pubkey, next) {
                                        log::error!(
                                            "Failed to register WireGuard peer (tag: {} {}): {}",
                                            tag,
                                            wg_params.keys.pubkey,
                                            e
                                        );
                                        return Err(format!(
                                            "Failed to register WireGuard peer (tag: {} {}): {}",
                                            tag,
                                            wg_params.keys.pubkey,
                                            e,));
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

                                                   println!(
                                    r#"
╔════════════════════════════════════╗
║    Wireguard Connection  Details   ║
╚════════════════════════════════════╝
"#);
                                                    println!("{}", conn);
                                                    let qrcode = QrCode::new(conn).unwrap();

                                                    let image = qrcode
                                                         .render::<unicode::Dense1x2>()
                                                        .quiet_zone(true)
                                                       // .module_dimensions(100, 100)
                                                        .build();
                                                    println!("{}\n", image);
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    log::warn!("WG API client is not available");
                                    return Err("WG API client is not available".into());
                                }
                            }
                        }
                        Ok(())
                    } => {},
                    _ = shutdown.recv() => {},
                }
            }
        });

        tasks.push(conn_task);
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

        let node_type = if settings.wg.enabled && settings.xray.enabled {
            NodeType::All
        } else if settings.wg.enabled {
            NodeType::Wireguard
        } else if settings.xray.enabled {
            NodeType::Xray
        } else {
            panic!("At least Wg or Xray should be enabled");
        };

        if !settings.agent.local {
            let _ = {
                let settings = settings.clone();
                log::debug!("Register node task {:?}", node_type);
                if let Err(e) = agent
                    .register_node(
                        settings.api.endpoint.clone(),
                        settings.api.token.clone(),
                        node_type,
                    )
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

    if settings.agent.metrics_enabled {
        log::info!("Running metrics send task");
        let carbon_addr = settings.carbon.address.clone();
        let metrics_handle: JoinHandle<()> = tokio::spawn({
            let agent = agent.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        _ = sleep(Duration::from_secs(settings.agent.metrics_interval)) => {
                            match agent.send_metrics(&carbon_addr).await {
                                Ok(_) => {},
                                Err(e) => log::error!("❌ Failed to send metrics: {}", e),
                            }
                        },
                        _ = shutdown.recv() => {
                            log::info!("🛑 Metrics task received shutdown");
                            break;
                        },
                    }
                }
            }
        });
        tasks.push(metrics_handle);

        log::info!("Running HB metrics send task");
        let carbon_addr = settings.carbon.address.clone();
        let metrics_handle = tokio::spawn({
            let agent = agent.clone();
            let mut shutdown = shutdown_tx.subscribe();
            async move {
                loop {
                    tokio::select! {
                        _ = sleep(Duration::from_secs(settings.agent.metrics_hb_interval)) => {
                            match agent.send_hb_metric(&carbon_addr).await {
                                Ok(_) => {},
                                Err(e) => log::error!("❌ Failed to send heartbeat: {}", e),
                            }
                        },
                        _ = shutdown.recv() => {
                            log::info!("🛑 Heartbeat task received shutdown");
                            break;
                        },
                    }
                }
            }
        });
        tasks.push(metrics_handle);
    }

    if settings.agent.stat_enabled {
        log::info!("Running Stat Task");
        let xray_stats_task = tokio::spawn({
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
        tasks.push(xray_stats_task);
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
