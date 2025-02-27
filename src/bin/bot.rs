use clap::Parser;
use fern::Dispatch;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;

use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        InlineKeyboardButton, InlineKeyboardMarkup, InlineQueryResultArticle, InputMessageContent,
        InputMessageContentText, Me,
    },
    utils::command::BotCommands,
};

use pony::{create_vpn_user, postgres_client, register, BotSettings, Settings};
use pony::{utils::*, ApiSettings};

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
#[command(version = "0.0.12", about = "pony tg-Bot")]
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

    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(
            Update::filter_message().endpoint(move |bot: Bot, msg: Message, me: Me| {
                let client = pg_client.clone();
                let settings = settings.clone();
                async move { message_handler(bot, msg, me, client, settings).await }
            }),
        )
        .branch(Update::filter_callback_query().endpoint(callback_handler))
        .branch(Update::filter_inline_query().endpoint(inline_query_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

/// Creates a keyboard made by buttons in a big column.
fn make_keyboard() -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    let actions = ["Vless", "Vmess"];

    for action in actions.chunks(2) {
        let row = action
            .iter()
            .map(|&action| InlineKeyboardButton::callback(action.to_owned(), action.to_owned()))
            .collect();

        keyboard.push(row);
    }

    InlineKeyboardMarkup::new(keyboard)
}

/// Parse the text wrote on Telegram and check if that text is a valid command
/// or not, then match the command. If the command is `/start` it writes a
/// markup with the `InlineKeyboardMarkup`.
async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
    client: Arc<Mutex<Client>>,
    settings: BotSettings,
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
                        match register(&username, client).await {
                            Ok(_) => {
                                let reply = format!("УСПЕХ {}", username);
                                if let Ok(_user) = create_vpn_user(
                                    username,
                                    settings.api.endpoint,
                                    settings.api.token,
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
                let keyboard = make_keyboard();
                bot.send_message(msg.chat.id, "Ok")
                    .reply_markup(keyboard)
                    .await?;
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
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let choose_debian_version = InlineQueryResultArticle::new(
        "0",
        "Chose debian version",
        InputMessageContent::Text(InputMessageContentText::new("")),
    )
    .reply_markup(make_keyboard());

    bot.answer_inline_query(q.id, vec![choose_debian_version.into()])
        .await?;

    Ok(())
}

/// When it receives a callback from a button it edits the message with all
/// those buttons writing a text with the selected Debian version.
///
/// **IMPORTANT**: do not send privacy-sensitive data this way!!!
/// Anyone can read data stored in the callback button.
async fn callback_handler(bot: Bot, q: CallbackQuery) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(ref version) = q.data {
        let text = format!("You chose: {version}");

        println!("{:?}", q);

        // Tell telegram that we've seen this query, to remove 🕑 icons from the
        // clients. You could also use `answer_callback_query`'s optional
        // parameters to tweak what happens on the client side.
        bot.answer_callback_query(&q.id).await?;

        // Edit text of the message to which the buttons were attached
        if let Some(message) = q.clone().regular_message() {
            bot.edit_message_text(message.chat.id, message.id, text)
                .await?;
        } else if let Some(id) = q.inline_message_id {
            bot.edit_message_text_inline(id, text).await?;
        }

        log::info!("You chose: {}", version);
    }

    Ok(())
}
