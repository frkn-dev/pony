use clap::Parser;
use clickhouse::Client;
use fern::Dispatch;
use log::info;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::{
    config::settings::{ApiSettings, Settings},
    http,
    http::debug::start_ws_server,
    state::state::State,
    utils::{current_timestamp, human_readable_date, level_from_settings},
    zmq::publisher::publisher,
};

#[derive(Parser)]
#[command(
    version = "0.1.1",
    about = "Pony Api - control tool for Xray/Wireguard"
)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
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
        .chain(fern::log_file(&settings.logging.file).unwrap())
        .apply()
        .unwrap();

    let debug = settings.debug.enabled;
    let ch_client = Arc::new(Client::default().with_url(&settings.clickhouse.address));
    let publisher = publisher(&settings.zmq.pub_endpoint).await;

    let state = {
        info!("Running User State Sync");

        Arc::new(Mutex::new(State::new()))
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

    tokio::spawn(http::api::run_api_server(state.clone(), publisher.clone()));
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for event");

    Ok(())
}
