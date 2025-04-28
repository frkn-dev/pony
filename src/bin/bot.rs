use std::error::Error;
use std::sync::Arc;
use teloxide::types::Me;

use clap::Parser;
use fern::Dispatch;
use pony::postgres::DbContext;

use teloxide::prelude::*;

use pony::bot::handlers::Handlers;
use pony::bot::BotState;
use pony::utils::*;
use pony::{postgres::postgres::postgres_client, BotSettings, Settings};

#[derive(Parser)]
#[command(about = "pony tg-Bot")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    println!("Config file {:?}", args.config);

    let mut settings = BotSettings::new(&args.config);
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

    let bot = Bot::new(settings.api.token.clone());
    let bot_state = Arc::new(BotState::new(settings));

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint({
            let bot_state = Arc::clone(&bot_state);
            move |bot: Bot, msg: Message, me: Me| {
                let bot_state = Arc::clone(&bot_state);
                async move { bot_state.message_handler(bot, msg, me).await }
            }
        }))
        .branch(Update::filter_callback_query().endpoint({
            let bot_state = Arc::clone(&bot_state);
            move |bot: Bot, query: CallbackQuery| {
                let bot_state = Arc::clone(&bot_state);
                async move { bot_state.callback_handler(bot, query).await }
            }
        }));

    let dispatcher_task = tokio::spawn(async move {
        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
    });

    tokio::try_join!(dispatcher_task)?;

    Ok(())
}
