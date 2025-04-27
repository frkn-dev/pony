use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;

use tokio::time::Duration;

use clap::Parser;
use fern::Dispatch;
use futures::future::join_all;
use log::error;
use tokio::sync::Mutex;

use pony::{
    api::tasks::Tasks,
    config::settings::{ApiSettings, Settings},
    http::{self, debug::start_ws_server},
    postgres::{postgres::postgres_client, DbContext},
    utils::*,
    Api, ChContext, Node, State, ZmqPublisher,
};

#[derive(Parser)]
#[command(about = "Pony Api - control tool for Xray/Wireguard")]
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
    let ch = ChContext::new(&settings.clickhouse.address);
    let publisher = ZmqPublisher::new(&settings.zmq.endpoint).await;
    let state: ApiState = State::new();
    let state = Arc::new(Mutex::new(state));

    let api = Arc::new(Api::new(
        db.clone(),
        ch.clone(),
        publisher.clone(),
        state.clone(),
    ));

    let _ = {
        match measure_time(db.node().get_nodes(), "get_nodes()".to_string()).await {
            Ok(nodes) => {
                let futures: Vec<_> = nodes.into_iter().map(|node| api.add_node(node)).collect();

                if let Some(Err(e)) = measure_time(join_all(futures), "Add nodes".to_string())
                    .await
                    .into_iter()
                    .find(Result::is_err)
                {
                    error!("Error during node state initialization: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to fetch nodes from DB: {}", e);
                return Err(e);
            }
        }

        match measure_time(db.user().get_all_users(), "get_all_users".to_string()).await {
            Ok(users) => {
                let futures: Vec<_> = users.into_iter().map(|user| api.add_user(user)).collect();

                if let Some(Err(e)) = measure_time(join_all(futures), "Add users".to_string())
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
        let node_timeout = settings.api.node_health_check_timeout;
        let job_interval = Duration::from_secs(settings.api.healthcheck_interval);
        let api = api.clone();

        async move {
            loop {
                if let Err(e) = api.node_healthcheck(node_timeout).await {
                    error!("Healthcheck failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let _ = tokio::spawn({
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.user_limit_check_interval);

        async move {
            loop {
                if let Err(e) = api.check_user_uplink_limits().await {
                    error!("Check limits  failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let _ = tokio::spawn({
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.user_reactivate_interval);

        async move {
            loop {
                if let Err(e) = api.reactivate_trial_users().await {
                    error!("Reactivate trial users task failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
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
