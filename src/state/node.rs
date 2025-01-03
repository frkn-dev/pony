use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::Ipv4Addr};
use uuid::Uuid;

use super::{
    inbound::{Inbound, InboundResponse},
    tag::Tag,
};

#[derive(Clone, Debug, Deserialize, Serialize, Copy)]
pub enum NodeStatus {
    Online,
    Offline,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub env: String,
    pub hostname: String,
    pub ipv4: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: Uuid,
}

impl NodeRequest {
    pub fn as_node(self) -> Node {
        let now = Utc::now();
        Node {
            env: self.env,
            uuid: self.uuid,
            hostname: self.hostname,
            ipv4: self.ipv4,
            inbounds: self.inbounds,
            status: NodeStatus::Online,
            created_at: now,
            modified_at: now,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeResponse {
    pub env: String,
    pub hostname: String,
    pub ipv4: Ipv4Addr,
    pub uuid: Uuid,
    pub inbounds: HashMap<Tag, InboundResponse>,
    pub status: NodeStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub hostname: String,
    pub ipv4: Ipv4Addr,
    pub status: NodeStatus,
    pub inbounds: HashMap<Tag, Inbound>,
    pub env: String,
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

impl Node {
    pub fn new(
        inbounds: HashMap<Tag, Inbound>,
        hostname: String,
        ipv4: Ipv4Addr,
        env: String,
        uuid: Uuid,
    ) -> Self {
        let now = Utc::now();
        Self {
            hostname: hostname,
            inbounds: inbounds,
            status: NodeStatus::Online,
            ipv4: ipv4,
            env: env,
            uuid: uuid,
            created_at: now,
            modified_at: now,
        }
    }

    pub fn as_node_response(&self) -> NodeResponse {
        let inbound_response = self
            .inbounds
            .clone()
            .into_iter()
            .map(|inbound| (inbound.0, inbound.1.as_inbound_response()))
            .collect();

        NodeResponse {
            env: self.env.clone(),
            hostname: self.hostname.clone(),
            ipv4: self.ipv4,
            uuid: self.uuid.clone(),
            inbounds: inbound_response,
            status: self.status,
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
