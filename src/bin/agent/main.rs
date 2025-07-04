use fern::Dispatch;

use pony::config::settings::AgentSettings;
use pony::config::settings::Settings;
use pony::utils::*;

mod core;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "debug")]
    console_subscriber::init();

    let config_path = &std::env::args()
        .nth(1)
        .expect("required config path as an argument");
    println!("Config file {}", config_path);

    let mut settings = AgentSettings::new(config_path);

    if let Err(e) = settings.validate() {
        panic!("Wrong settings file {}", e);
    }
    println!(">>> Settings: {:?}", settings.clone());

    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                record.level(),
                human_readable_date(current_timestamp() as u64),
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
