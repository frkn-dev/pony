use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Message {
    pub user_id: Uuid,
    pub action: Action,
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_message = serde_json::to_string(self).expect("Failed to serialize message");

        write!(f, "{}", json_message)
    }
}
