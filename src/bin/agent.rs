use std::net::Ipv4Addr;
use std::sync::Arc;

use clap::Parser;
use fern::Dispatch;
use log::debug;
use log::error;
use log::info;
use uuid::Uuid;

use pony::xray_op::actions::create_users;
use pony::NodeStorage;
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};

use pony::{
    http::debug::start_ws_server, jobs::agent, utils::*, AgentSettings, HandlerClient, Node,
    Settings, State, StatsClient, Tag, XrayClient, XrayConfig, ZmqSubscriber,
};

#[derive(Parser)]
#[command(
    version = "0.0.23-dev",
    about = "Pony Agent - control tool for Xray/Wireguard"
)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

type AgentState = State<Node>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let args = Cli::parse();
    println!("Config file {:?}", args.config);

    // Settings
    let mut settings = AgentSettings::new(&args.config);

    if let Err(e) = settings.validate() {
        panic!("Wrong settings file {}", e);
    }
    println!(">>> Settings: {:?}", settings.clone());

    // Logs handler init
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                record.level(),
                human_readable_date(current_timestamp()),
                record.target(),
                message
            ))
        })
        .level(level_from_settings(&settings.logging.level))
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    let debug = settings.debug.enabled;
    let env = settings.node.env.clone();
    let node_uuid = settings.node.uuid.clone();

    let mut tasks: Vec<JoinHandle<()>> = vec![];

    // Xray-core Config Validation
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

    // User State
    let state = {
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
            env.clone(),
            settings
                .node
                .default_interface
                .clone()
                .unwrap_or("eth0".to_string()),
            node_uuid,
            settings.node.label.clone(),
        );

        let state: AgentState = State::with_node(node);
        let state = Arc::new(Mutex::new(state));
        state
    };

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

    // METRICS TASKS
    if settings.agent.metrics_enabled && !settings.agent.local {
        info!("Running metrics send job");
        let metrics_handle = tokio::spawn({
            let state = Arc::clone(&state);
            let settings = settings.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.agent.metrics_interval)).await;
                    let _ = agent::send_metrics_job(
                        Arc::clone(&state),
                        settings.carbon.address.clone(),
                    )
                    .await;
                }
            }
        });
        tasks.push(metrics_handle);
    }

    // ++ Recurent Jobs ++
    if settings.agent.stat_enabled && !settings.agent.local {
        // Statistics
        info!("Running Stat job");
        let stats_task = tokio::spawn({
            let state = state.clone();
            let stats_client = xray_stats_client.clone();
            let handler_client = xray_handler_client.clone();

            async move {
                loop {
                    tokio::task::yield_now().await;
                    sleep(Duration::from_secs(settings.agent.stat_job_interval)).await;
                    let _ = agent::collect_stats_job(
                        stats_client.clone(),
                        handler_client.clone(),
                        state.clone(),
                    )
                    .await;
                }
            }
        });
        tasks.push(stats_task);
    }

    if !settings.agent.local {
        // zeromq SUB messages listener
        info!("ZMQ task starting...");
        let zmq_task = tokio::spawn({
            let state = Arc::clone(&state);
            let client = Arc::clone(&xray_handler_client);
            let settings = settings.clone();

            async move {
                let sub = ZmqSubscriber::new(
                    &settings.zmq.sub_endpoint,
                    &settings.node.uuid,
                    &settings.node.env,
                );
                if let Err(e) = sub.run(client, settings, state).await {
                    error!("ZMQ subscriber error: {}", e);
                }
            }
        });

        tasks.push(zmq_task);

        let _ = {
            let settings = settings.clone();
            debug!("----->> Register node");
            if let Err(e) = agent::register_node(
                Arc::clone(&state),
                settings.api.endpoint,
                settings.api.token,
            )
            .await
            {
                panic!("Cannot register node {:?}", e);
            }
        };
    }

    let user_task = tokio::spawn({
        let username = Uuid::new_v4();

        async move {
            create_users(
                username,
                Some("password".to_string()),
                xray_handler_client.clone(),
            )
            .await
            .expect("Create local accounts FAILED");

            let state = state.lock().await;
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

    // Run all tasks
    let _ = futures::future::join_all(tasks).await;
    Ok(())
}
