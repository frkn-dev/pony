use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Tag;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub name: String,
    pub inbounds: Vec<Tag>,
    pub uplink: i64,
    pub downlink: i64,
    pub user_count: i64,
    pub user_count_online: i64,
}

impl Node {
    pub fn new(inbounds: Vec<Tag>) -> Self {
        Self {
            name: Uuid::new_v4().to_string(),
            inbounds: inbounds,
            uplink: 0,
            downlink: 0,
            user_count: 0,
            user_count_online: 0,
        }
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            name: Uuid::new_v4().to_string(),
            inbounds: vec![],
            uplink: 0,
            downlink: 0,
            user_count: 0,
            user_count_online: 0,
        }
    }
}
