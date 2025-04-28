use std::fmt;
use std::str::FromStr;
use std::{collections::HashMap, net::Ipv4Addr};

use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::tag::Tag;
use crate::config::settings::NodeConfig;
use crate::config::xray::{Config as XrayConfig, Inbound, InboundResponse};

#[derive(Clone, Debug, Deserialize, Serialize, Copy)]
pub enum NodeStatus {
    Online,
    Offline,
}

impl PartialEq for NodeStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NodeStatus::Online, NodeStatus::Online) => true,
            (NodeStatus::Offline, NodeStatus::Offline) => true,
            _ => false,
        }
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Online => write!(f, "Online"),
            NodeStatus::Offline => write!(f, "Offline"),
        }
    }
}

impl FromStr for NodeStatus {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Online" => Ok(NodeStatus::Online),
            "Offline" => Ok(NodeStatus::Offline),
            _ => Ok(NodeStatus::Offline),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: Uuid,
    pub label: String,
    pub interface: String,
}

impl NodeRequest {
    pub fn as_node(self) -> Node {
        let now = Utc::now();
        Node {
            uuid: self.uuid,
            env: self.env,
            hostname: self.hostname,
            address: self.address,
            inbounds: self.inbounds,
            status: NodeStatus::Online,
            created_at: now,
            modified_at: now,
            label: self.label,
            interface: self.interface,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeResponse {
    pub uuid: Uuid,
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, InboundResponse>,
    pub status: NodeStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub uuid: Uuid,
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub status: NodeStatus,
    pub label: String,
    pub interface: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub inbounds: HashMap<Tag, Inbound>,
}

impl Node {
    pub fn new(settings: NodeConfig, config: XrayConfig) -> Self {
        let now = Utc::now();
        let inbounds = config
            .inbounds
            .into_iter()
            .map(|inbound| (inbound.tag.clone(), inbound))
            .collect::<HashMap<Tag, Inbound>>();

        Self {
            uuid: settings.uuid,
            env: settings.env,
            hostname: settings.hostname.expect("hostname"),
            status: NodeStatus::Online,
            address: settings.address.expect("address"),
            created_at: now,
            label: settings.label,
            interface: settings.default_interface.expect("default_interface"),
            modified_at: now,
            inbounds: inbounds,
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
            address: self.address,
            uuid: self.uuid.clone(),
            inbounds: inbound_response,
            status: self.status,
        }
    }

    pub fn update_uplink(&mut self, tag: &Tag, new_uplink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(tag) {
            inbound.update_uplink(new_uplink);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
    }

    pub fn update_status(&mut self, new_status: NodeStatus) -> Result<(), String> {
        self.status = new_status;
        Ok(())
    }

    pub fn update_downlink(&mut self, tag: &Tag, new_downlink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(tag) {
            inbound.update_downlink(new_downlink);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
    }

    pub fn update_conn_count(&mut self, tag: &Tag, conn_count: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(tag) {
            inbound.update_conn_count(conn_count);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
    }
}
