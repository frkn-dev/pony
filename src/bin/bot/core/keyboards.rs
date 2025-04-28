use async_trait::async_trait;
use reqwest::Url;
use std::collections::HashMap;

use teloxide::types::InlineKeyboardButton;
use teloxide::types::InlineKeyboardMarkup;

use super::BotState;

#[async_trait]
pub trait Keyboards {
    async fn conn_keyboard(&self, conns: Vec<String>) -> InlineKeyboardMarkup;
}

#[async_trait]
impl Keyboards for BotState {
    async fn conn_keyboard(&self, conns: Vec<String>) -> InlineKeyboardMarkup {
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
            let mut map_lock = self.callback_map.lock().await;
            map_lock.extend(new_entries);
        }
        InlineKeyboardMarkup::new(keyboard)
    }
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
