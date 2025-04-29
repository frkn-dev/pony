use std::collections::HashMap;
use std::sync::Arc;
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;

use pony::config::settings::BotSettings;

pub mod handlers;
mod http;
mod keyboards;

#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Display this text
    Help,
    /// Start
    Start,
    /// Reg
    Register,
    //Get connection line
    Connect,
}

pub struct BotState {
    pub settings: BotSettings,
    pub callback_map: Arc<Mutex<HashMap<String, String>>>,
}

impl BotState {
    pub fn new(settings: BotSettings) -> Self {
        Self {
            settings,
            callback_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
