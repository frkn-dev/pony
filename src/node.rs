use serde::{Deserialize, Serialize};

use std::{collections::HashMap, net::Ipv4Addr};
use uuid::Uuid;

use crate::xray_op::Tag;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Inbound {
    port: i16,
    uplink: Option<i64>,
    downlink: Option<i64>,
    user_count: Option<i64>,
}

impl Inbound {
    pub fn new(port: i16) -> Self {
        Self {
            port: port,
            uplink: Some(0),
            downlink: Some(0),
            user_count: Some(0),
        }
    }
    pub fn update_uplink(&mut self, new_uplink: i64) {
        self.uplink = Some(new_uplink);
    }

    pub fn update_downlink(&mut self, new_downlink: i64) {
        self.downlink = Some(new_downlink);
    }
    pub fn update_user_count(&mut self, new_user_count: i64) {
        self.user_count = Some(new_user_count);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub hostname: String,
    pub ipv4: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub uuid: String,
    pub hostname: String,
    pub ipv4: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
}

impl Node {
    pub fn new(inbounds: HashMap<Tag, Inbound>, hostname: String, ipv4: Ipv4Addr) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            hostname: hostname,
            inbounds: inbounds,
            ipv4: ipv4,
        }
    }

    pub fn as_node_request(self) -> NodeRequest {
        NodeRequest {
            hostname: self.hostname,
            ipv4: self.ipv4,
            inbounds: self.inbounds,
            uuid: Some(self.uuid),
        }
    }

    pub fn update_uplink(&mut self, tag: Tag, new_uplink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(&tag) {
            inbound.update_uplink(new_uplink);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
    }

    pub fn update_downlink(&mut self, tag: Tag, new_downlink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(&tag) {
            inbound.update_downlink(new_downlink);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
    }

    pub fn update_user_count(&mut self, tag: Tag, user_count: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(&tag) {
            inbound.update_user_count(user_count);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
    }
}