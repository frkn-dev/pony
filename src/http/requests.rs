use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::config::wireguard::WireguardSettings;
use crate::config::xray::Inbound;
use crate::config::xray::StreamSettings;
use crate::memory::connection::wireguard::Param as WgParam;
use crate::memory::node::Node;
use crate::memory::node::Status as NodeStatus;
use crate::memory::tag::ProtoTag as Tag;

fn default_format() -> String {
    "txt".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserIdQueryParam {
    pub id: uuid::Uuid,
}
#[derive(Debug, Deserialize)]
pub struct UserSubQueryParam {
    pub id: uuid::Uuid,
    #[serde(default = "default_format")]
    pub format: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserConnQueryParam {
    pub user_id: uuid::Uuid,
    pub limit: i32,
    pub trial: bool,
    pub password: Option<String>,
    pub env: String,
}

#[derive(Serialize, Deserialize)]
pub struct NodesQueryParams {
    pub env: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeIdParam {
    pub id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnQueryParam {
    pub id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum NodeType {
    Xray,
    Wireguard,
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeTypeParam {
    pub node_type: Option<NodeType>,
    pub last_update: Option<u64>,
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
    pub cores: usize,
    pub max_bandwidth_bps: i64,
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
            cores: self.cores,
            max_bandwidth_bps: self.max_bandwidth_bps,
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
    pub cores: usize,
    pub max_bandwidth_bps: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InboundResponse {
    pub tag: Tag,
    pub port: u16,
    pub stream_settings: Option<StreamSettings>,
    pub wg: Option<WireguardSettings>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnCreateRequest {
    pub env: String,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
    pub proto: Tag,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnUpdateRequest {
    pub env: Option<String>,
    pub password: Option<String>,
    pub is_deleted: Option<bool>,
}
