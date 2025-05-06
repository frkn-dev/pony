use clap::Parser;
use fern::Dispatch;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Me;

use pony::config::settings::BotSettings;
use pony::config::settings::Settings;
use pony::utils::*;
use pony::Result;

use crate::core::handlers::Handlers;
use crate::core::http::ApiRequests;
use crate::core::BotState;

mod core;

#[derive(Parser)]
#[command(about = "pony tg-Bot")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
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

    let bot = Bot::new(settings.bot.token.clone());
    let mut bot_state = BotState::new(settings);

    if let Ok(users) = bot_state.get_users().await {
        log::debug!("Users {:?}", users);
        bot_state.add_users(Arc::new(users)).await;
    } else {
        panic!("Cannot get users");
    }

    let bot_state = Arc::new(bot_state);

    log::debug!("!!BotState {:?}", bot_state);

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
