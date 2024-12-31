use clap::Parser;
use fern::Dispatch;
use futures::future::join_all;
use log::error;
use log::info;

use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};

use pony::{
    http::debug::start_ws_server,
    jobs,
    metrics::metrics::MetricType,
    node::Node,
    postgres::{postgres_client, users_db_request},
    settings::AgentSettings,
    settings::Settings,
    state::State,
    utils::measure_time,
    utils::{current_timestamp, human_readable_date, level_from_settings},
    xray_op::{client::XrayClients, config},
    zmq::subscriber::subscriber,
};

#[derive(Parser)]
#[command(
    version = "0.1.0",
    about = "Pony Agent - control tool for Xray/Wireguard"
)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

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
        .chain(fern::log_file(&settings.logging.file).unwrap())
        .apply()
        .unwrap();

    let debug = settings.debug.enabled;
    let env = settings.node.env.clone();
    let node_uuid = settings.node.uuid.clone();

    let mut tasks: Vec<JoinHandle<()>> = vec![];

    // Clients
    let pg_client = match postgres_client(settings.pg.clone()).await {
        Ok(client) => client,
        Err(e) => panic!("PG not available, {}", e),
    };

    // Xray-core Config Validation
    let xray_config = match config::read_xray_config(&settings.xray.xray_config_path) {
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
    let xray_api_clients = match XrayClients::new(xray_api_endpoint).await {
        Ok(clients) => clients,
        Err(e) => panic!("Can't create clients: {}", e),
    };

    // User State
    let state = {
        info!("Running User State Sync");

        let inbounds = xray_config.get_inbounds();
        let node = Node::new(
            inbounds,
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
            node_uuid,
        );

        let mut state = State::new();
        if let Err(e) = state.add_node(node).await {
            error!("Failed to add node: {}", e);
            return Err(e);
        }
        let state = Arc::new(Mutex::new(state));

        match measure_time(
            users_db_request(pg_client.clone(), env.clone()),
            "db query".to_string(),
        )
        .await
        {
            Ok(users) => {
                let futures: Vec<_> = users
                    .into_iter()
                    .map(|user| {
                        jobs::init_state(
                            state.clone(),
                            settings.clone(),
                            xray_api_clients.clone(),
                            user,
                            debug,
                        )
                    })
                    .collect();

                if let Some(Err(e)) = measure_time(join_all(futures), "Init state".to_string())
                    .await
                    .into_iter()
                    .find(Result::is_err)
                {
                    error!("Error during user state initialization: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to fetch users from DB: {}", e);
                return Err(e);
            }
        }

        state
    };

    if debug {
        tokio::spawn(start_ws_server(
            state.clone(),
            settings
                .debug
                .web_server
                .unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
            settings.debug.web_port,
        ));
    }

    // ++ Recurent Jobs ++
    if settings.app.stat_enabled {
        // Statistics
        info!("Running Stat job");
        let stats_task = tokio::spawn({
            let state = state.clone();
            let clients = xray_api_clients.clone();
            let env = env.clone();
            async move {
                loop {
                    sleep(Duration::from_secs(settings.app.stat_jobs_timeout)).await;
                    let _ = jobs::collect_stats_job(
                        clients.clone(),
                        state.clone(),
                        settings.node.uuid,
                        env.clone(),
                    )
                    .await;
                }
            }
        });
        tasks.push(stats_task);
    }

    if settings.app.trial_users_enabled {
        // Block trial users by traffic limit
        info!("Running trial users limit by traffic job");
        let block_trial_users_by_limit_handle = tokio::spawn({
            let state = state.clone();
            let clients = xray_api_clients.clone();
            async move {
                loop {
                    sleep(Duration::from_secs(settings.app.trial_jobs_timeout)).await;
                    jobs::block_trial_users_by_limit(
                        state.clone(),
                        clients.clone(),
                        env.clone(),
                        node_uuid.clone(),
                    )
                    .await;
                }
            }
        });
        tasks.push(block_trial_users_by_limit_handle);

        // Restore trial user
        info!("Running restoring trial users job");
        let restore_trial_users_handle = tokio::spawn({
            let state = state.clone();
            let clients = xray_api_clients.clone();
            async move {
                loop {
                    jobs::restore_trial_users(state.clone(), clients.clone()).await;
                    sleep(Duration::from_secs(settings.app.trial_jobs_timeout)).await;
                }
            }
        });
        tasks.push(restore_trial_users_handle);
    }

    // zeromq SUB messages listener
    let _ = {
        let settings = settings.clone();
        let user_state = state.clone();
        tasks.push(tokio::spawn(subscriber(
            xray_api_clients.clone(),
            settings.clone(),
            user_state,
            debug,
        )))
    };

    // METRICS TASKS
    if settings.app.metrics_enabled {
        info!("Running metrics send job");
        let metrics_handle = tokio::spawn({
            let state = state.clone();
            let settings = settings.clone();

            async move {
                loop {
                    sleep(Duration::from_secs(settings.app.metrics_timeout)).await;
                    let _ = jobs::send_metrics_job::<MetricType>(
                        state.clone(),
                        settings.clone(),
                        node_uuid.clone(),
                    )
                    .await;
                }
            }
        });
        tasks.push(metrics_handle);
    }

    // Run all tasks
    let _ = futures::future::try_join_all(tasks).await;
    Ok(())
}