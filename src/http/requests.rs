use crate::config::xray::Inbound;
use crate::config::xray::StreamSettings;
use crate::state::node::Node;
use crate::state::node::NodeStatus;
use crate::state::tag::Tag;
use crate::zmq::message::Action;
use crate::zmq::message::Message;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserQueryParam {
    pub username: String,
}

#[derive(Serialize, Deserialize)]
pub struct NodesQueryParams {
    pub env: String,
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InboundResponse {
    pub port: u16,
    pub stream_settings: Option<StreamSettings>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnRequest {
    pub conn_id: uuid::Uuid,
    pub action: Action,
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

impl ConnRequest {
    pub fn as_message(&self) -> Message {
        Message {
            conn_id: self.conn_id,
            action: self.action.clone(),
            password: self.password.clone(),
        }
    }
}
