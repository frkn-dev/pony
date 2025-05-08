use async_trait::async_trait;
use std::collections::HashMap;

use teloxide::types::InlineKeyboardButton;
use teloxide::types::InlineKeyboardMarkup;

use pony::state::ConnStat;

use super::BotState;

use pony::http::requests::NodeResponse;
use pony::state::Conn;
use pony::state::Tag;

#[async_trait]
pub trait Keyboards {
    async fn conn_keyboard(
        &self,
        conns: Vec<(uuid::Uuid, Conn, NodeResponse, Tag)>,
    ) -> InlineKeyboardMarkup;
    fn format_traffic_stats(&self, stats: Vec<(uuid::Uuid, ConnStat)>, limit: i32) -> String;
}

#[async_trait]
impl Keyboards for BotState {
    async fn conn_keyboard(
        &self,
        conns: Vec<(uuid::Uuid, Conn, NodeResponse, Tag)>,
    ) -> InlineKeyboardMarkup {
        let mut keyboard = Vec::new();
        let mut new_entries = HashMap::new();

        log::debug!("conn_keyboard {:?}", conns);

        for (i, (conn_id, conn, node, tag)) in conns.iter().enumerate() {
            if conn.proto != *tag {
                continue;
            }

            let key = format!("conn_{}", i);
            let label = &node.label;
            let id_str = conn_id.to_string();
            let first_octet = id_str.split('-').next().unwrap_or("");

            new_entries.insert(key.clone(), (*conn_id, conn.clone(), node.clone(), *tag));

            keyboard.push(vec![InlineKeyboardButton::callback(
                format!("{} | {} | id: {}", tag, label, first_octet),
                key,
            )]);
        }

        {
            let mut map_lock = self.callback_map.lock().await;
            map_lock.extend(new_entries);
        }

        InlineKeyboardMarkup::new(keyboard)
    }

    fn format_traffic_stats(&self, stats: Vec<(uuid::Uuid, ConnStat)>, limit: i32) -> String {
        if stats.is_empty() {
            return "Нет активных подключений.".to_string();
        }

        let mut out = String::from("📊 *Статистика трафика за сегодня:*\n\n");

        for (conn_id, stat) in stats {
            let status = if stat.downlink as f64 / 1_048_576.0 >= limit.into() {
                "Expired".to_string()
            } else {
                "Active".to_string()
            };
            let block = format!(
                "🔹  Status: {status} \n `{}`\n  • Uplink: {:.0} MB\n  • Downlink: {:.0} / {limit} MB\n Devices Online: {}\n\n",
                conn_id,
                stat.uplink as f64 / 1_048_576.0,
                stat.downlink as f64 / 1_048_576.0,
                stat.online
            );
            out.push_str(&block);
        }
        out
    }
}
