use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::Tag;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Inbound {
    uplink: i64,
    downlink: i64,
    user_count: i64,
}

impl Inbound {
    pub fn new() -> Self {
        Self {
            uplink: 0,
            downlink: 0,
            user_count: 0,
        }
    }
    pub fn update_uplink(&mut self, new_uplink: i64) {
        self.uplink = new_uplink;
    }

    pub fn update_downlink(&mut self, new_downlink: i64) {
        self.downlink = new_downlink;
    }
    pub fn update_user_count(&mut self, new_user_count: i64) {
        self.user_count = new_user_count;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub name: String,
    pub inbounds: HashMap<Tag, Inbound>,
}

impl Node {
    pub fn new(inbounds: HashMap<Tag, Inbound>) -> Self {
        Self {
            name: Uuid::new_v4().to_string(),
            inbounds: inbounds,
        }
    }

    pub fn update_uplink(&mut self, tag: Tag, new_uplink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(&tag) {
            inbound.update_uplink(new_uplink);
            Ok(())
        } else {
            Err("Tag not found".to_string())
        }
    }

    pub fn update_downlink(&mut self, tag: Tag, new_downlink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(&tag) {
            inbound.update_downlink(new_downlink);
            Ok(())
        } else {
            Err("Tag not found".to_string())
        }
    }

    pub fn update_user_count(&mut self, tag: Tag, user_count: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(&tag) {
            inbound.update_user_count(user_count);
            Ok(())
        } else {
            Err("Tag not found".to_string())
        }
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            name: Uuid::new_v4().to_string(),
            inbounds: HashMap::new(),
        }
    }
}
