use async_trait::async_trait;

use teloxide::{payloads::SendMessageSetters, prelude::*, types::Me, utils::command::BotCommands};

use super::Command;

use pony::Result;

use super::keyboards::Keyboards;
use super::BotState;
use crate::core::http::ApiRequests;
use crate::core::http::RegisterStatus;

#[async_trait]
pub trait Handlers {
    async fn message_handler(&self, bot: Bot, msg: Message, me: Me) -> Result<()>;

    async fn callback_handler(&self, bot: Bot, q: CallbackQuery) -> Result<()>;
}

#[async_trait]
impl Handlers for BotState {
    async fn message_handler(&self, bot: Bot, msg: Message, me: Me) -> Result<()> {
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
                            match self.register_user(&username).await {
                                Ok(RegisterStatus::Ok) => {
                                    let reply = format!("Спасибо за регистрацию {}", username);
                                    bot.send_message(msg.chat.id, reply).await?;
                                }
                                Ok(RegisterStatus::AlreadyExist) => {
                                    let reply = format!(
                                        "Уже зарегистрирован, используй комманду /connect  {}",
                                        username
                                    );
                                    bot.send_message(msg.chat.id, reply).await?;
                                }
                                Err(e) => {
                                    log::error!("Command::Register error {}", e);
                                    let reply = format!("Упс, ошибка {}", username);
                                    bot.send_message(msg.chat.id, reply).await?;
                                }
                            }
                        }
                    }
                }

                Ok(Command::Connect) => {
                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            let res = self.get_user_vpn_connection(&username).await;
                            match res {
                                Ok(Some(connection_info)) => {
                                    let response = "Выбери VPN кофигурацию".to_string();

                                    let keyboard = self.conn_keyboard(connection_info).await;

                                    bot.send_message(msg.chat.id, response)
                                        .reply_markup(keyboard)
                                        .await?;
                                }
                                Ok(None) => {
                                    bot.send_message(
                                        msg.chat.id,
                                        "Нет ни одной конфигурации. Создаем, попробуйте /connect через пару минут ",
                                    )
                                    .await?;
                                }
                                Err(e) => {
                                    log::error!("VPN conn error: {:?}", e);
                                    bot.send_message(
                                        msg.chat.id,
                                        "Не удалось получить подключение",
                                    )
                                    .await?;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    bot.send_message(msg.chat.id, "Command not found!").await?;
                }
            }
        }

        Ok(())
    }

    async fn callback_handler(&self, bot: Bot, q: CallbackQuery) -> Result<()> {
        println!("Callback data {:?}", q.data);

        if let Some(ref key) = q.data {
            let map_lock = self.callback_map.lock().await;
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
}
