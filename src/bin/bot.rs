use clap::Parser;
use fern::Dispatch;
use log::debug;
use reqwest::Url;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;

use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        InlineKeyboardButton, InlineKeyboardMarkup, InlineQuery, InlineQueryResultArticle,
        InputMessageContent, InputMessageContentText, Me,
    },
    utils::command::BotCommands,
};

use pony::utils::*;
use pony::{
    jobs::bot::{create_vpn_user, get_conn, register},
    user_exist,
};
use pony::{postgres_client, BotSettings, Settings};

#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Display this text
    Help,
    /// Start
    Start,
    /// Reg
    Register,
    ///Get Vpn
    Getvpn,
    /// Referal
    Refferal,
    /// Payment
    Payment,
}

#[derive(Parser)]
#[command(version = "0.0.14", about = "pony tg-Bot")]
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

    let pg_client = match postgres_client(settings.pg.clone()).await {
        Ok(client) => client,
        Err(e) => panic!("PG not available, {}", e),
    };

    let settings = Arc::new(Mutex::new(settings));

    let bot = Bot::from_env();

    let callback_map: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint({
            let client = Arc::clone(&pg_client);
            let settings = Arc::clone(&settings);
            let callback_map = Arc::clone(&callback_map);
            move |bot: Bot, msg: Message, me: Me| {
                let client = Arc::clone(&client);
                let settings = Arc::clone(&settings);
                let callback_map = Arc::clone(&callback_map);
                async move { message_handler(bot, msg, me, client, settings, callback_map).await }
            }
        }))
        .branch(Update::filter_callback_query().endpoint({
            let callback_map = Arc::clone(&callback_map);
            move |bot: Bot, q: CallbackQuery| {
                let callback_map = Arc::clone(&callback_map);
                async move { callback_handler(bot, q, callback_map).await }
            }
        }))
        .branch(Update::filter_inline_query().endpoint({
            let client = Arc::clone(&pg_client);
            let settings = Arc::clone(&settings);
            let callback_map = Arc::clone(&callback_map);
            move |bot: Bot, q: InlineQuery| {
                let client = Arc::clone(&client);
                let settings = Arc::clone(&settings);
                let callback_map = Arc::clone(&callback_map);
                async move { inline_query_handler(bot, q, client, settings, callback_map).await }
            }
        }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
    client: Arc<Mutex<Client>>,
    settings: Arc<Mutex<BotSettings>>,
    callback_map: Arc<Mutex<HashMap<String, String>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        match BotCommands::parse(text, me.username()) {
            Ok(Command::Help) => {
                // Just send the description of all commands.
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
            Ok(Command::Start) => {
                // Create a list of buttons and send them.
                bot.send_message(msg.chat.id, "Пожалуйста зарегистрируйтесь")
                    .await?;
            }

            Ok(Command::Register) => {
                // Handle user registration.
                if let Some(user) = msg.from {
                    if let Some(username) = user.username {
                        let user_id = uuid::Uuid::new_v4();
                        match register(&username, user_id, client).await {
                            Ok(_) => {
                                let settings = settings.lock().await;
                                let reply = format!("УСПЕХ {}", username);
                                if let Ok(_user) = create_vpn_user(
                                    username,
                                    user_id,
                                    settings.api.endpoint.clone(),
                                    settings.api.token.clone(),
                                )
                                .await
                                {
                                    bot.send_message(msg.chat.id, reply).await?;
                                }
                            }
                            Err(_) => {
                                let reply = format!("Упс, ошибка при регистрации {}", username);
                                bot.send_message(msg.chat.id, reply).await?;
                            }
                        }
                    }
                }
            }
            Ok(Command::Getvpn) => {
                if let Some(user) = msg.from {
                    if let Some(username) = user.username {
                        if let Some(user_id) =
                            user_exist(client.clone(), username.to_string()).await
                        {
                            let settings = settings.lock().await;

                            if let Ok(conns) = get_conn(
                                user_id,
                                settings.api.endpoint.clone(),
                                settings.api.token.clone(),
                            )
                            .await
                            {
                                let keyboard = make_keyboard(conns.clone(), callback_map).await;
                                debug!("Conns {:?}", conns);
                                bot.send_message(msg.chat.id, "Выбери VPN")
                                    .reply_markup(keyboard)
                                    .await?;
                            }
                        }
                    }
                }
            }
            Ok(Command::Refferal) => {
                bot.send_message(msg.chat.id, "Поделитесь с другом").await?;
            }
            Ok(Command::Payment) => {
                bot.send_message(msg.chat.id, "Ссылка на оплату").await?;
            }

            Err(_) => {
                bot.send_message(msg.chat.id, "Command not found!").await?;
            }
        }
    }

    Ok(())
}

async fn inline_query_handler(
    bot: Bot,
    q: InlineQuery,
    client: Arc<Mutex<Client>>,
    settings: Arc<Mutex<BotSettings>>,
    callback_map: Arc<Mutex<HashMap<String, String>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(user_id) = user_exist(client.clone(), q.from.username.unwrap()).await {
        let settings = settings.lock().await;

        if let Ok(conns) = get_conn(
            user_id,
            settings.api.endpoint.clone(),
            settings.api.token.clone(),
        )
        .await
        {
            // Вызываем make_keyboard, который возвращает HashMap
            let keyboard = make_keyboard(conns, callback_map.clone()).await;

            println!(
                "callback_map after update: {:?}",
                *callback_map.lock().await
            );

            let choose_vpn = InlineQueryResultArticle::new(
                "0",
                "Choose vpn version",
                InputMessageContent::Text(InputMessageContentText::new("Choose VPN ")),
            )
            .reply_markup(keyboard);

            bot.answer_inline_query(q.id, vec![choose_vpn.into()])
                .await?;
        }
    }

    Ok(())
}

async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    callback_map: Arc<Mutex<HashMap<String, String>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(ref key) = q.data {
        let map_lock = callback_map.lock().await;
        if let Some(conn) = map_lock.get(key) {
            let text = format!("🔗 Ваша VPN ссылка:\n{}", conn);

            bot.answer_callback_query(&q.id).await?;

            if let Some(message) = q.clone().regular_message() {
                bot.edit_message_text(message.chat.id, message.id, text)
                    .await?;
            } else if let Some(id) = q.inline_message_id {
                bot.edit_message_text_inline(id, text).await?;
            }

            log::info!("User chose: {}", conn);
        }
    }

    Ok(())
}
fn extract_info(conn: &str) -> (String, String) {
    if let Ok(url) = Url::parse(conn) {
        let scheme = url.scheme().to_uppercase(); // "VLESS", "VMESS"
        let name = url.fragment().unwrap_or("UNKNOWN").to_string(); // "VLESS-XTLS" или "VLESS-GRPC"
        (scheme, name)
    } else {
        ("UNKNOWN".to_string(), "INVALID".to_string())
    }
}

async fn make_keyboard(
    conns: Vec<String>,
    callback_map: Arc<Mutex<HashMap<String, String>>>,
) -> InlineKeyboardMarkup {
    let mut keyboard = Vec::new();
    let mut new_entries = HashMap::new();

    for (i, conn) in conns.iter().enumerate() {
        let key = format!("conn_{}", i);
        let (protocol, name) = extract_info(conn);
        new_entries.insert(key.clone(), conn.clone());

        keyboard.push(vec![InlineKeyboardButton::callback(
            format!("{} - {}", protocol, name),
            key,
        )]);
    }

    {
        let mut map_lock = callback_map.lock().await;
        map_lock.extend(new_entries);
    }

    println!("Updated callback_map: {:?}", callback_map.lock().await);

    InlineKeyboardMarkup::new(keyboard)
}
