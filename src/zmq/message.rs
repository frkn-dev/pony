use serde::{Deserialize, Serialize};
use std::fmt;

use crate::state::connection::wireguard::Param as WgParam;
use crate::state::tag::Tag;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")] // COMMENT(qezz): I think you can rename_all with snake_case
    Create,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "reset_stat")]
    ResetStat,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Message {
    pub conn_id: uuid::Uuid,
    pub action: Action,
    pub tag: Tag,
    pub wg: Option<WgParam>,
    pub password: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_message = serde_json::to_string(self).expect("Failed to serialize message");
        write!(f, "{}", json_message)
    }
}
