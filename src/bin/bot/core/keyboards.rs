use async_trait::async_trait;
use pony::state::ConnStatus;
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
    fn format_traffic_stats(&self, data: Vec<(uuid::Uuid, ConnStat, Tag, ConnStatus)>, limit: i32) -> String;
}

#[async_trait]
impl Keyboards for BotState {
    async fn conn_keyboard(
        &self,
        conns: Vec<(uuid::Uuid, Conn, NodeResponse, Tag)>,
    ) -> InlineKeyboardMarkup {
        let mut keyboard = Vec::new();
        let mut new_entries = HashMap::new();

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

    fn format_traffic_stats(&self, data: Vec<(uuid::Uuid, ConnStat, Tag, ConnStatus)>, limit: i32) -> String {
        if data.is_empty() {
            return "Нет активных подключений.".to_string();
        }

        let mut out = String::from("📊 *Статистика трафика за сегодня:*\n\n");

        let mut total_uplink = 0;
        let mut total_downlink = 0;

        for (conn_id, stat, proto, conn_status) in &data {
            total_uplink += stat.uplink;
            total_downlink += stat.downlink;

            let block = format!(
                "🔹 {} \n id: `{}` \n\n Status: {conn_status}\n\n • Upload: {:.0} MB\n • Download: {:.0}   MB\n • Devices Online: {}\n\n",
            
                proto,
                conn_id,
                stat.uplink as f64 / 1_048_576.0,
                stat.downlink as f64 / 1_048_576.0,
                stat.online
            );
            out.push_str(&block);
        }

        let status = if total_downlink as f64 / 1_048_576.0 >= limit as f64 {
            "Expired"
        } else {
            "Active"
        };

        out.push_str(&format!(
            "🔻 *Суммарно:* \n Status: {status}\n\n↑ Upload {:.0} MB\n↓ Download {:.0} MB\n\n  *Download Limit:* {limit}  MB\n",
            total_uplink as f64 / 1_048_576.0,
            total_downlink as f64 / 1_048_576.0
        ));

        out
    }
}
