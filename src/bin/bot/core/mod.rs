use std::collections::HashMap;
use std::sync::Arc;
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;

use pony::config::settings::BotSettings;
use pony::http::requests::NodeResponse;
use pony::state::Conn;
use pony::state::Tag;
use pony::state::User;

pub mod handlers;
pub(crate) mod http;
mod keyboards;

#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Помощь
    Help,
    /// Старт
    Start,
    /// Cтоп
    Stop,
    ///Получить ссылки для подключения
    Connect,
    /// Subscription link
    Sub,
    ///Получить статистику
    Limit,
}

type CallbackMap = Arc<Mutex<HashMap<String, (uuid::Uuid, Conn, NodeResponse, Tag)>>>;

#[derive(Debug)]
pub struct BotState {
    pub settings: BotSettings,
    pub callback_map: CallbackMap,
    pub users: Arc<Mutex<HashMap<String, uuid::Uuid>>>,
}

impl BotState {
    pub fn new(settings: BotSettings) -> Self {
        Self {
            settings,
            callback_map: Arc::new(Mutex::new(HashMap::new())),
            users: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    pub async fn add_users(&mut self, users: Arc<Vec<(uuid::Uuid, User)>>) {
        let mut user_map: HashMap<String, uuid::Uuid> = HashMap::new();
        for (id, user) in users.iter() {
            user_map.insert(user.username.clone(), *id);
        }
        self.users = Arc::clone(&Arc::new(Mutex::new(user_map.clone())));
    }
}

#[async_trait::async_trait]
pub trait UserStorage {
    async fn add_user(&self, username: &str, user_id: uuid::Uuid);
}

#[async_trait::async_trait]
impl UserStorage for BotState {
    async fn add_user(&self, username: &str, user_id: uuid::Uuid) {
        let users = &mut self.users.lock().await;
        users.insert(username.to_string(), user_id);
    }
}
