use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::config::h2::H2Settings;
use crate::config::wireguard::WireguardSettings;
use crate::config::xray::Inbound;
use crate::config::xray::StreamSettings;
use crate::memory::connection::wireguard::Param as WgParam;
use crate::memory::node::Node;
use crate::memory::node::Status as NodeStatus;
use crate::memory::tag::ProtoTag as Tag;

fn default_format() -> String {
    "plain".to_string()
}

fn default_env() -> String {
    "dev".to_string()
}

fn default_proto() -> TagReq {
    TagReq::Xray
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TagReq {
    Xray,
    Wireguard,
    Hysteria2,
    Mtproto,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubIdQueryParam {
    pub id: uuid::Uuid,
    pub env: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubQueryParam {
    pub id: uuid::Uuid,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_env")]
    pub env: String,
    #[serde(default = "default_proto")]
    pub proto: TagReq,
}

#[derive(Debug, Deserialize)]
pub struct MtprotoQueryParam {
    pub id: uuid::Uuid,
    #[serde(default = "default_env")]
    pub env: String,
}

#[derive(Debug, Deserialize)]
pub struct SubCreateReq {
    pub referred_by: Option<String>,
    pub refer_code: Option<String>,
    pub days: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SubUpdateReq {
    pub referred_by: Option<String>,
    pub days: Option<i64>,
    pub bonus_days: Option<i32>,
    pub refer_code: Option<String>,
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
pub struct ConnTypeParam {
    pub proto: Tag,
    pub last_update: Option<u64>,
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
    pub h2: Option<H2Settings>,
    pub mtproto_secret: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnCreateRequest {
    pub env: String,
    pub token: Option<uuid::Uuid>,
    pub password: Option<String>,
    pub subscription_id: Option<uuid::Uuid>,
    pub proto: Tag,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
    pub days: Option<u16>,
}

impl ConnCreateRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.proto == Tag::Mtproto {
            return Err("Mtproto is not supported for Connection".into());
        }
        if self.password.is_some() && self.wg.is_some() {
            return Err("Cannot specify both password and wg".into());
        }
        if self.token.is_some() && self.wg.is_some() {
            return Err("Cannot specify both token and wg".into());
        }
        if self.token.is_some() && self.password.is_some() {
            return Err("Cannot specify both token and password".into());
        }
        if !self.proto.is_wireguard() && self.wg.is_some() {
            return Err("Wg params only allowed for Wireguard".into());
        }

        if !self.proto.is_wireguard() && self.node_id.is_some() {
            return Err("node_id only allowed for Wireguard".into());
        }
        if self.proto.is_shadowsocks() && self.password.is_none() {
            return Err("Password required for Shadowsocks".into());
        }
        if !self.proto.is_shadowsocks() && self.password.is_some() {
            return Err("Password only allowed for Shadowsocks".into());
        }
        if !self.proto.is_hysteria2() && self.token.is_some() {
            return Err("Token only allowed for Hysteria2".into());
        }
        if self.proto.is_hysteria2() && self.token.is_none() {
            return Err("Token required for Hysteria2".into());
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnUpdateRequest {
    pub env: Option<String>,
    pub password: Option<String>,
    pub is_deleted: Option<bool>,
    pub days: Option<i64>,
}
