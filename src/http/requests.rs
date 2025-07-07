use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::config::wireguard::WireguardSettings;
use crate::config::xray::Inbound;
use crate::config::xray::StreamSettings;
use crate::state::connection::wireguard::Param as WgParam;
use crate::state::node::Node;
use crate::state::node::Status as NodeStatus;
use crate::state::tag::Tag;
use crate::state::user::User;
use crate::ConnectionStatus;

fn default_format() -> String {
    "txt".to_string()
}

#[derive(Deserialize, Debug)]
pub struct UserReq {
    pub telegram_id: Option<u64>,
    pub username: String,
    pub env: String,
    pub limit: Option<i32>,
    pub password: Option<String>,
}

impl From<UserReq> for User {
    fn from(req: UserReq) -> Self {
        let now = Utc::now().naive_utc();

        User {
            telegram_id: req.telegram_id,
            username: req.username,
            env: req.env,
            limit: req.limit,
            password: req.password,
            created_at: now,
            modified_at: now,
            is_deleted: false,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct UserUpdateReq {
    pub env: Option<String>,
    pub limit: Option<i32>,
    pub password: Option<String>,
    pub is_deleted: Option<bool>,
}
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
    pub node_id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnQueryParam {
    pub conn_id: uuid::Uuid,
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
    pub wg: Option<WireguardSettings>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnCreateRequest {
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i32>,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
    pub proto: Tag,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnUpdateRequest {
    pub env: Option<String>,
    pub trial: Option<bool>,
    pub limit: Option<i32>,
    pub password: Option<String>,
    pub is_deleted: Option<bool>,
    pub status: Option<ConnectionStatus>,
}
