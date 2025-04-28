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
    api::http::routes::Http, api::tasks::Tasks, http::debug::start_ws_server,
    postgres::postgres_client, utils::*, Api, ApiSettings, ChContext, Node, PgContext, Settings,
    State, ZmqPublisher,
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

    let db = PgContext::new(pg_client);
    let ch = ChContext::new(&settings.clickhouse.address);
    let publisher = ZmqPublisher::new(&settings.zmq.endpoint).await;
    let state: ApiState = State::new();
    let state = Arc::new(Mutex::new(state));

    let api = Arc::new(Api::new(
        db.clone(),
        ch.clone(),
        publisher.clone(),
        state.clone(),
        settings.clone(),
    ));

    let _ = {
        match measure_time(db.node().all(), "get all nodes()".to_string()).await {
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

        match measure_time(db.conn().all(), "get all connections from db".to_string()).await {
            Ok(conns) => {
                let futures: Vec<_> = conns.into_iter().map(|conn| api.add_conn(conn)).collect();

                if let Some(Err(e)) = measure_time(
                    join_all(futures),
                    "add all connections to state".to_string(),
                )
                .await
                .into_iter()
                .find(Result::is_err)
                {
                    error!("Error during conn state initialization: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to fetch conns from DB: {}", e);
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
        let job_interval = Duration::from_secs(settings.api.healthcheck_interval);
        let api = api.clone();

        async move {
            loop {
                if let Err(e) = api.node_healthcheck().await {
                    error!("Healthcheck failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let _ = tokio::spawn({
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.conn_limit_check_interval);

        async move {
            loop {
                if let Err(e) = api.check_conn_uplink_limits().await {
                    error!("Check limits  failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let _ = tokio::spawn({
        let api = api.clone();
        let job_interval = Duration::from_secs(settings.api.conn_reactivate_interval);

        async move {
            loop {
                if let Err(e) = api.reactivate_trial_conns().await {
                    error!("Reactivate trial conns task failed: {:?}", e);
                }
                tokio::time::sleep(job_interval).await;
            }
        }
    });

    let api = api.clone();
    tokio::spawn(async move { api.run().await });
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for event");

    Ok(())
}
