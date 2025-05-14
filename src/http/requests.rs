use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::config::xray::Inbound;
use crate::config::xray::StreamSettings;
use crate::state::Node;
use crate::state::NodeStatus;
use crate::state::Tag;
use crate::zmq::message::Action;
use crate::zmq::message::Message;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserIdQueryParam {
    pub user_id: uuid::Uuid,
}

#[derive(Debug, Deserialize)]
pub struct UserSubQueryParam {
    pub user_id: uuid::Uuid,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "txt".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct NodesQueryParams {
    pub env: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserConnQueryParam {
    pub user_id: uuid::Uuid,
    pub limit: i32,
    pub trial: bool,
    pub password: Option<String>,
    pub env: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnQueryParam {
    pub conn_id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: uuid::Uuid,
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
    pub uuid: uuid::Uuid,
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, InboundResponse>,
    pub status: NodeStatus,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InboundResponse {
    pub tag: Tag,
    pub port: u16,
    pub stream_settings: Option<StreamSettings>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnRequest {
    pub conn_id: uuid::Uuid,
    pub action: Action,
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i32>,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
    pub proto: Tag,
}

impl ConnRequest {
    pub fn as_message(&self) -> Message {
        Message {
            conn_id: self.conn_id,
            action: self.action.clone(),
            password: self.password.clone(),
            tag: self.proto,
        }
    }
}
