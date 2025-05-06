use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "reset_stat")]
    ResetStat,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Message {
    pub conn_id: uuid::Uuid,
    pub action: Action,
    pub password: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_message = serde_json::to_string(self).expect("Failed to serialize message");
        write!(f, "{}", json_message)
    }
}
