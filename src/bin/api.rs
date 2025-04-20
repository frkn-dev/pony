use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;

use tokio::time::Duration;

use clap::Parser;
use clickhouse::Client as ChClient;
use fern::Dispatch;
use futures::future::join_all;
use log::{debug, error};
use tokio::sync::Mutex;

use pony::{
    config::settings::{ApiSettings, Settings},
    http,
    http::debug::start_ws_server,
    jobs::api,
    postgres::{postgres::postgres_client, DbContext},
    utils::*,
    Node, State, ZmqPublisher,
};

#[derive(Parser)]
#[command(
    version = "0.0.23-dev",
    about = "Pony Api - control tool for Xray/Wireguard"
)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

type ApiState = State<HashMap<String, Vec<Node>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let args = Cli::parse();

    println!("Config file {:?}", args.config);

    let mut settings = ApiSettings::new(&args.config);

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

    let pg_client = match postgres_client(settings.pg.clone()).await {
        Ok(client) => client,
        Err(e) => panic!("PG not available, {}", e),
    };

    let db = DbContext::new(pg_client);

    let ch_client = Arc::new(Mutex::new(
        ChClient::default().with_url(&settings.clickhouse.address),
    ));
    let publisher = ZmqPublisher::new(&settings.zmq.pub_endpoint).await;

    let state = {
        let state: ApiState = State::new();
        let state = Arc::new(Mutex::new(state));

        match measure_time(db.node().get_nodes(), "get_nodes()".to_string()).await {
            Ok(nodes) => {
                let futures: Vec<_> = nodes
                    .into_iter()
                    .map(|node| api::init_state(state.clone(), node))
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

        match measure_time(db.user().get_all_users(), "db query".to_string()).await {
            Ok(users) => {
                let futures: Vec<_> = users
                    .into_iter()
                    .map(|user| api::sync_users(state.clone(), user))
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

    let _ = tokio::spawn({
        let state = state.clone();
        let ch_client = ch_client.clone();
        let db = db.clone();
        let timeout = settings.api.node_health_check_timeout;
        let job_timeout = settings.api.healthcheck_interval;
        let interval = Duration::from_secs(job_timeout);

        async move {
            loop {
                if let Err(e) =
                    api::node_healthcheck(state.clone(), ch_client.clone(), db.clone(), timeout)
                        .await
                {
                    error!("Healthcheck failed: {:?}", e);
                }

                tokio::time::sleep(interval).await;
            }
        }
    });

    tokio::spawn(http::api::run_api_server(
        state.clone(),
        db,
        publisher.clone(),
        settings.api.address.unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
        settings.api.port,
        settings.api.token,
        settings.api.user_limit_mb,
    ));
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for event");

    Ok(())
}
