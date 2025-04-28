use async_trait::async_trait;
use std::error::Error;

use teloxide::{payloads::SendMessageSetters, prelude::*, types::Me, utils::command::BotCommands};

use crate::api::requests::ApiRequests;

use super::BotState;
use super::Command;

#[async_trait]
pub trait Handlers {
    async fn message_handler(
        &self,
        bot: Bot,
        msg: Message,
        me: Me,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn callback_handler(
        &self,
        bot: Bot,
        q: CallbackQuery,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
impl Handlers for BotState {
    async fn message_handler(
        &self,
        bot: Bot,
        msg: Message,
        me: Me,
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
                            match self.register_user(&username).await {
                                Ok(_) => {
                                    let reply = format!("Спасибо за регистрацию {}", username);

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

                Err(_) => {
                    bot.send_message(msg.chat.id, "Command not found!").await?;
                }
            }
        }

        Ok(())
    }

    async fn callback_handler(
        &self,
        bot: Bot,
        q: CallbackQuery,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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
