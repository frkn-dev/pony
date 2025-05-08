use clap::Parser;
use fern::Dispatch;

use pony::config::settings::AgentSettings;
use pony::config::settings::Settings;
use pony::utils::*;

mod core;

#[derive(Parser)]
#[command(
    about = "Pony Agent - control tool for Xray/Wireguard",
    version = "v0.1.11"
)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let args = Cli::parse();
    println!("Config file {:?}", args.config);

    let mut settings = AgentSettings::new(&args.config);

    if let Err(e) = settings.validate() {
        panic!("Wrong settings file {}", e);
    }
    println!(">>> Settings: {:?}", settings.clone());

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

    let num_cpus = std::thread::available_parallelism()?.get();

    let worker_threads = if num_cpus <= 1 { 1 } else { num_cpus * 2 };
    log::info!(
        "🧠 CPU cores: {}, configured worker threads: {}",
        num_cpus,
        worker_threads
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(core::service::run(settings))?;

    Ok(())
}
