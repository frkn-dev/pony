use async_trait::async_trait;
use pony::http::requests::NodeResponse;
use std::collections::HashSet;
use teloxide::types::ParseMode;
use teloxide::{payloads::SendMessageSetters, prelude::*, types::Me, utils::command::BotCommands};

use super::Command;
use super::UserStorage;

use pony::state::{Conn, ConnStat, ConnStatus, NodeStatus};
use pony::utils::create_conn_link;
use pony::Result;

use super::keyboards::Keyboards;
use super::BotState;
use crate::core::http::ApiRequests;

#[async_trait]
pub trait Handlers {
    async fn message_handler(&self, bot: Bot, msg: Message, me: Me) -> Result<()>;
    async fn callback_handler(&self, bot: Bot, q: CallbackQuery) -> Result<()>;
}

#[async_trait]
impl Handlers for BotState {
    async fn message_handler(&self, bot: Bot, msg: Message, me: Me) -> Result<()> {
        let env = &self.settings.bot.env;
        if let Some(text) = msg.text() {
            match BotCommands::parse(text, me.username()) {
                Ok(Command::Help) => {
                    // Just send the description of all commands.
                    bot.send_message(msg.chat.id, Command::descriptions().to_string())
                        .await?;
                }
                Ok(Command::Start) => {
                    bot.send_message(msg.chat.id, "Пожалуйста зарегистрируйтесь /register")
                        .await?;
                }

                Ok(Command::Register) => {
                    // Handle user registration.
                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            match self.register_user(&username).await {
                                Ok(Some(user_id)) => {
                                    let reply = format!("Спасибо за регистрацию {}", username);
                                    let _ = self.add_user(&username, user_id).await;

                                    bot.send_message(msg.chat.id, reply).await?;
                                }
                                Ok(None) => {
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

                // Handle connections
                Ok(Command::Connect) => {
                    let user_map = self.users.lock().await;

                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            if let Some(user_id) = user_map.get(&username) {
                                let mut conns = match self.get_user_vpn_connections(&user_id).await
                                {
                                    Ok(Some(c)) => c,
                                    _ => vec![],
                                };

                                let existing_protos: HashSet<_> =
                                    conns.iter().map(|(_, c)| c.proto).collect();

                                let nodes: Result<Option<Vec<NodeResponse>>> =
                                    if let Ok(Some(nodes)) = self.get_nodes(env).await {
                                        Ok(Some(nodes))
                                    } else {
                                        Ok(None)
                                    };

                                if let Ok(Some(ref nodes)) = nodes {
                                    let mut available_protos = HashSet::new();
                                    for node in
                                        nodes.iter().filter(|n| n.status == NodeStatus::Online)
                                    {
                                        for tag in node.inbounds.keys() {
                                            available_protos.insert(*tag);
                                        }
                                    }

                                    for tag in available_protos.difference(&existing_protos) {
                                        let conn_id = uuid::Uuid::new_v4();
                                        let conn = Conn::new(
                                            true,
                                            self.settings.bot.daily_limit_mb,
                                            env,
                                            ConnStatus::Active,
                                            None,
                                            Some(*user_id),
                                            ConnStat::default(),
                                            *tag,
                                        );

                                        log::debug!("conns.push {} {}", conn_id, conn.proto);
                                        conns.push((conn_id, conn));
                                    }
                                }

                                let mut connections = vec![];

                                for (conn_id, conn) in conns.iter() {
                                    if let Ok(Some(ref nodes)) = nodes {
                                        for node in
                                            nodes.iter().filter(|n| n.status == NodeStatus::Online)
                                        {
                                            if let Some(_inbound) = node.inbounds.get(&conn.proto) {
                                                log::debug!(
                                                    "connections.push {} {} {}",
                                                    conn_id,
                                                    conn.proto,
                                                    node.uuid
                                                );
                                                connections.push((
                                                    *conn_id,
                                                    conn.clone(),
                                                    node.clone(),
                                                    conn.proto,
                                                ));
                                            } else {
                                                log::debug!(
                                                    "SKIP node {} does not support proto {}",
                                                    node.uuid,
                                                    conn.proto
                                                );
                                            }
                                        }
                                    }
                                }
                                let response = if connections.is_empty() {
                                    "Что-то пошло не так".to_string()
                                } else {
                                    "Выбери VPN кофигурацию".to_string()
                                };

                                let keyboard = self.conn_keyboard(connections).await;
                                bot.send_message(msg.chat.id, response)
                                    .reply_markup(keyboard)
                                    .await?;
                            } else {
                                bot.send_message(msg.chat.id, "Нужно зарегистрироваться /register")
                                    .await?;
                            }
                        }
                    }
                }

                // Handle user stat
                Ok(Command::Limit) => {
                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            let user_map = self.users.lock().await;
                            if let Some(user_id) = user_map.get(&username) {
                                let stats = self.get_user_traffic_stat(&user_id).await;
                                match stats {
                                    Ok(Some(stats)) => {
                                        let stat_str = self.format_traffic_stats(
                                            stats,
                                            self.settings.bot.daily_limit_mb,
                                        );
                                        bot.send_message(msg.chat.id, stat_str).await?;
                                    }

                                    Ok(None) => {
                                        bot.send_message(
                                            msg.chat.id,
                                            "Не удалось найти статистику, используй /connect ",
                                        )
                                        .await?;
                                    }

                                    Err(e) => {
                                        log::error!("Stat conn error: {:?}", e);
                                        bot.send_message(
                                            msg.chat.id,
                                            "Не удалось найти статистику, используй /connect ",
                                        )
                                        .await?;
                                    }
                                }
                            } else {
                                bot.send_message(msg.chat.id, "Нужно зарегистрироваться /register")
                                    .await?;
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
        if let Some(ref key) = q.data {
            let map_lock = self.callback_map.lock().await;

            if let Some((conn_id, conn, node, tag)) = map_lock.get(key) {
                let inbound = match node.inbounds.get(tag) {
                    Some(i) => i,
                    None => {
                        log::warn!("No inbound found for tag {:?} on node {:?}", tag, node.uuid);
                        return Ok(());
                    }
                };

                let label = &node.label;
                let address = node.address;
                let env = &node.uuid.to_string();
                let trial = conn.trial;
                let limit = conn.limit;
                let user_id = match conn.user_id {
                    Some(id) => id,
                    None => {
                        log::warn!("No user_id in connection {:?}", conn_id);
                        return Ok(());
                    }
                };
                let proto = conn.proto;

                if let Ok(link) = create_conn_link(*tag, conn_id, inbound.clone(), label, address) {
                    let text = format!("🔗 Ваша VPN ссылка:\n\n`{}`", link);

                    bot.answer_callback_query(&q.id).await?;

                    let _ = self
                        .post_create_or_update_connection(
                            conn_id, &user_id, trial, limit, env, proto,
                        )
                        .await;

                    if let Some(message) = q.clone().regular_message() {
                        bot.edit_message_text(message.chat.id, message.id, text)
                            .parse_mode(ParseMode::MarkdownV2)
                            .await?;
                    } else if let Some(id) = q.inline_message_id {
                        bot.edit_message_text_inline(id, text)
                            .parse_mode(ParseMode::MarkdownV2)
                            .await?;
                    }

                    log::info!("User chose VPN link: {}", link);
                } else {
                    log::warn!("Failed to build link for tag {:?} on conn {}", tag, conn_id);
                }
            } else {
                log::warn!("No callback mapping found for key: {}", key);
            }
        }

        Ok(())
    }
}
