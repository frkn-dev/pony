use async_trait::async_trait;
use pony::http::requests::NodeResponse;
use std::collections::HashSet;
use teloxide::sugar::request::RequestLinkPreviewExt;
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

        let reply1 = format!(
            "Добро пожаловать в *FRKN*\nЭтот бот выдаёт ссылки на подключение к *VPN* \n\n"
        );
        let reply2 = format!("Kлиенты:\n*Android*: Hiddify\n*Windows*: Hiddify, Clash Verge\n");
        let reply3 = format!(
            "*iOS, MacOS*: Clash Verge, Streisand, Foxray, Shadowrocket\n*Linux*: Clash Verge\n"
        );
        let reply4 = format!(
            "Больше [клиентов](https://github.com/XTLS/Xray-core?tab=readme-ov-file#gui-clients)\n"
        );

        let reply5 =
            format!("\nДоступно в сутки 1024 Мегабайта, команда /limit для просмотра статистики");
        let reply6 = format!("\n\nПолучить *VPN* /connect или /sub");

        let welcome_msg = format!("{reply1}\n{reply2}{reply3}\n{reply4}\n{reply5}\n{reply6}");

        if let Some(text) = msg.text() {
            match BotCommands::parse(text, me.username()) {
                Ok(Command::Help) => {
                    // Just send the description of all commands.
                    bot.send_message(msg.chat.id, Command::descriptions().to_string())
                        .await?;
                }

                // Handle user registration.
                Ok(Command::Start) => {
                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            match self
                                .register_user_req(
                                    &username,
                                    Some((user.id.0 as i64).try_into().unwrap()),
                                )
                                .await
                            {
                                Ok(Some(user_id)) => {
                                    if let Some(result) = self.add_user(&username, user_id).await {
                                        log::debug!("Couldn't add user {} {:?}", user_id, result);
                                    }

                                    bot.send_message(msg.chat.id, welcome_msg)
                                        .disable_link_preview(true)
                                        .parse_mode(ParseMode::MarkdownV2)
                                        .await?;
                                }
                                Ok(None) => {
                                    let reply = format!(
                                        "Для получения VPN Используй комманду /connect или /sub",
                                    );
                                    bot.send_message(msg.chat.id, reply).await?;
                                }
                                Err(e) => {
                                    log::error!("Command::Start error {}", e);
                                    let reply = format!("Упс, ошибка");
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
                            if let Some((user_id, deleted)) = user_map.get(&username) {
                                if *deleted {
                                    bot.send_message(msg.chat.id, "Для начала /start").await?;
                                    return Ok(());
                                }
                                let mut conns =
                                    match self.get_user_vpn_connections_req(&user_id).await {
                                        Ok(Some(c)) => c,
                                        _ => vec![],
                                    };

                                let existing_protos: HashSet<_> =
                                    conns.iter().map(|(_, c)| c.proto).collect();

                                let nodes: Result<Option<Vec<NodeResponse>>> =
                                    if let Ok(Some(nodes)) = self.get_nodes_req(env).await {
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
                                                connections.push((
                                                    *conn_id,
                                                    conn.clone(),
                                                    node.clone(),
                                                    conn.proto,
                                                ));
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
                                bot.send_message(msg.chat.id, "Для начала /start").await?;
                            }
                        }
                    }
                }

                // Handle Subscriptions link
                Ok(Command::Sub) => {
                    let user_map = self.users.lock().await;

                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            if let Some((user_id, deleted)) = user_map.get(&username) {
                                if *deleted {
                                    bot.send_message(msg.chat.id, "Для начала /start").await?;
                                    return Ok(());
                                }
                                let conns = match self.get_user_vpn_connections_req(&user_id).await
                                {
                                    Ok(Some(c)) => c,
                                    _ => vec![],
                                };

                                if conns.is_empty() {
                                    if let Err(e) =
                                        self.post_create_all_connection_req(user_id).await
                                    {
                                        log::error!("Cannot create connections {}", e);
                                    }
                                }

                                let sub_clash_link = format!(
                                    "`{}/sub?user_id={}&format=clash`",
                                    self.settings.api.endpoint, user_id
                                );

                                let sub_link = format!(
                                    "`{}/sub?user_id={}`",
                                    self.settings.api.endpoint, user_id
                                );

                                let response1 = format!("*Default Subscription Link*\n  Клиенты: Hiddify, Streisand, Foxray\n\n{sub_link}\n\n");
                                let response2 = format!("*Clash Subscription Link*  \n  Клиенты: Clash Verge, Shadowrocket\n\n{sub_clash_link}");

                                let response3 = format!("Скопируй ссылку и импортируй её в клиенте, ссылка автоматически обновляет доступные подключения");

                                let response = format!("{response1} {response2}\n\n {response3}");

                                bot.send_message(msg.chat.id, response)
                                    .parse_mode(ParseMode::MarkdownV2)
                                    .await?;
                            } else {
                                bot.send_message(msg.chat.id, "Для начала /start").await?;
                            }
                        }
                    }
                }

                // Handle user stat
                Ok(Command::Limit) => {
                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            let user_map = self.users.lock().await;
                            if let Some((user_id, deleted)) = user_map.get(&username) {
                                if *deleted {
                                    bot.send_message(msg.chat.id, "Для начала /start").await?;
                                    return Ok(());
                                }
                                let stats = self.get_user_traffic_stat_req(&user_id).await;
                                match stats {
                                    Ok(Some(data)) => {
                                        let stat_str = self.format_traffic_stats(
                                            data,
                                            self.settings.bot.daily_limit_mb,
                                        );
                                        bot.parse_mode(ParseMode::MarkdownV2)
                                            .send_message(msg.chat.id, stat_str)
                                            .await?;
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
                                bot.send_message(msg.chat.id, "Для начала /start").await?;
                            }
                        }
                    }
                }
                Ok(Command::Stop) => {
                    if let Some(user) = msg.from {
                        if let Some(username) = user.username {
                            let mut user_map = self.users.lock().await;
                            if let Some((user_id, is_deleted)) = user_map.get_mut(&username) {
                                if *is_deleted == true {
                                    bot.send_message(msg.chat.id, "Для начала /start")
                                        .disable_link_preview(true)
                                        .parse_mode(ParseMode::MarkdownV2)
                                        .await?;
                                    return Ok(());
                                }
                                *is_deleted = true;

                                if let Err(e) = self.delete_user_req(user_id).await {
                                    log::error!("DELETE /user error {}", e);
                                }
                                bot.send_message(msg.chat.id, "Спасибо что был с нами, удачи")
                                    .disable_link_preview(true)
                                    .parse_mode(ParseMode::MarkdownV2)
                                    .await?;
                            }
                        }
                    }
                }

                Err(_) => {
                    bot.send_message(msg.chat.id, welcome_msg)
                        .disable_link_preview(true)
                        .parse_mode(ParseMode::MarkdownV2)
                        .await?;
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
                let env = &self.settings.bot.env;
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
                        .post_create_or_update_connection_req(
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
