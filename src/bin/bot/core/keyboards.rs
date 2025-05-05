use async_trait::async_trait;
use reqwest::Url;
use std::collections::HashMap;

use teloxide::types::InlineKeyboardButton;
use teloxide::types::InlineKeyboardMarkup;

use pony::state::ConnStat;

use super::BotState;

#[async_trait]
pub trait Keyboards {
    async fn conn_keyboard(&self, conns: Vec<String>) -> InlineKeyboardMarkup;
    fn extract_info(conn: &str) -> (String, String);
    fn format_traffic_stats(&self, stats: Vec<(uuid::Uuid, ConnStat)>) -> String;
}

#[async_trait]
impl Keyboards for BotState {
    async fn conn_keyboard(&self, conns: Vec<String>) -> InlineKeyboardMarkup {
        let mut keyboard = Vec::new();
        let mut new_entries = HashMap::new();

        for (i, conn) in conns.iter().enumerate() {
            let key = format!("conn_{}", i);
            let (protocol, name) = Self::extract_info(conn);
            new_entries.insert(key.clone(), conn.clone());

            keyboard.push(vec![InlineKeyboardButton::callback(
                format!("{} - {}", protocol, name),
                key,
            )]);
        }

        {
            let mut map_lock = self.callback_map.lock().await;
            map_lock.extend(new_entries);
        }
        InlineKeyboardMarkup::new(keyboard)
    }

    fn extract_info(conn: &str) -> (String, String) {
        if let Ok(url) = Url::parse(conn) {
            let scheme = url.scheme().to_uppercase();
            let name = url.fragment().unwrap_or("UNKNOWN").to_string();

            let name = urlencoding::decode(&name)
                .map(|cow| cow.into_owned())
                .unwrap_or(name);
            (scheme, name)
        } else {
            ("UNKNOWN".to_string(), "INVALID".to_string())
        }
    }

    fn format_traffic_stats(&self, stats: Vec<(uuid::Uuid, ConnStat)>) -> String {
        if stats.is_empty() {
            return "Нет активных подключений.".to_string();
        }

        let mut out = String::from("📊 *Статистика трафика:*\n\n");

        for (conn_id, stat) in stats {
            let block = format!(
                "🔹 `{}`\n  • Uplink: {} MB\n  • Downlink: {} MB\n\n\n",
                conn_id,
                stat.uplink as f64 / 1_048_576.0,
                stat.downlink as f64 / 1_048_576.0,
            );
            out.push_str(&block);
        }

        out
    }
}
