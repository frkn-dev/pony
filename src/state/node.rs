use chrono::DateTime;
use chrono::Utc;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::{collections::HashMap, net::Ipv4Addr};
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

use super::tag::Tag;
use crate::config::xray::{Config, Inbound, InboundResponse};

#[derive(Clone, Debug, Deserialize, Serialize, Copy)]
pub enum NodeStatus {
    Online,
    Offline,
    Unknown,
}

impl PartialEq for NodeStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NodeStatus::Online, NodeStatus::Online) => true,
            (NodeStatus::Offline, NodeStatus::Offline) => true,
            (NodeStatus::Unknown, NodeStatus::Unknown) => true,
            _ => false,
        }
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Online => write!(f, "Online"),
            NodeStatus::Offline => write!(f, "Offline"),
            NodeStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

impl FromStr for NodeStatus {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Online" => Ok(NodeStatus::Online),
            "Offline" => Ok(NodeStatus::Offline),
            _ => Ok(NodeStatus::Unknown),
        }
    }
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
    pub fn new(config: Config, hostname: String, ipv4: Ipv4Addr, env: String, uuid: Uuid) -> Self {
        let now = Utc::now();
        let inbounds = config
            .inbounds
            .into_iter()
            .map(|inbound| (inbound.tag.clone(), inbound))
            .collect::<HashMap<Tag, Inbound>>();
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

    pub async fn insert_node(client: Arc<Mutex<Client>>, node: Node) -> Result<(), Box<dyn Error>> {
        let client = client.lock().await;

        let query = "
            INSERT INTO nodes (hostname, ipv4, status, inbounds, env, uuid, created_at, modified_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ";

        let status_str = node.status.to_string();
        let ip: IpAddr = IpAddr::V4(node.ipv4);
        let inbounds_json = serde_json::to_value(&node.inbounds)?;

        let result = client
            .execute(
                query,
                &[
                    &node.hostname,
                    &ip,
                    &status_str,
                    &inbounds_json,
                    &node.env,
                    &node.uuid,
                    &node.created_at,
                    &node.modified_at,
                ],
            )
            .await;
        match result {
            Ok(_) => debug!("Node inserted successfully"),
            Err(e) => error!("Error inserting node: {}", e),
        }

        Ok(())
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

    pub fn update_status(&mut self, new_status: NodeStatus) -> Result<(), String> {
        self.status = new_status;
        Ok(())
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
