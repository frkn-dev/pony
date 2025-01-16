use clap::Parser;
use clickhouse::Client;
use fern::Dispatch;
use futures::future::join_all;
use log::{debug, error};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::{
    config::settings::{ApiSettings, Settings},
    http,
    http::debug::start_ws_server,
    jobs::api,
    postgres::{node::nodes_db_request, postgres::postgres_client, user::users_db_request},
    state::state::State,
    utils::{current_timestamp, human_readable_date, level_from_settings, measure_time},
    zmq::publisher::publisher,
};

#[derive(Parser)]
#[command(
    version = "0.0.8-dev",
    about = "Pony Api - control tool for Xray/Wireguard"
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

    let ch_client = Arc::new(Mutex::new(
        Client::default().with_url(&settings.clickhouse.address),
    ));
    let publisher = publisher(&settings.zmq.pub_endpoint).await;

    let state = {
        let state = State::new();
        let state = Arc::new(Mutex::new(state));

        match measure_time(nodes_db_request(pg_client.clone()), "db query".to_string()).await {
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

        match measure_time(
            users_db_request(pg_client.clone(), None),
            "db query".to_string(),
        )
        .await
        {
            Ok(users) => {
                let futures: Vec<_> = users
                    .into_iter()
                    .map(|user| api::sync_users(state.clone(), user, 0))
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

    debug!("STATE: {:?}", state);

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

    tokio::spawn(api::node_healthcheck(
        state.clone(),
        ch_client,
        settings.api.node_health_check_timeout,
    ));

    tokio::spawn(http::api::run_api_server(
        state.clone(),
        pg_client.clone(),
        publisher.clone(),
        settings.api.address.unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
        settings.api.port,
        settings.api.token,
    ));
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for event");

    Ok(())
}
