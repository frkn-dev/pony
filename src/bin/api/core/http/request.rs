use pony::config::xray::Inbound;
use pony::memory::connection::wireguard::Param as WgParam;
use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::memory::tag::ProtoTag as Tag;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TagReq {
    Xray,
    Wireguard,
    Hysteria2,
    Mtproto,
}

#[derive(Debug, Deserialize)]
pub struct Subscription {
    pub referred_by: Option<String>,
    pub refer_code: Option<String>,
    pub days: Option<i64>,
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
    pub fn as_node(&self) -> Node {
        let now = Utc::now();
        Node {
            uuid: self.uuid,
            env: self.env.clone(),
            hostname: self.hostname.clone(),
            address: self.address,
            inbounds: self.inbounds.clone(),
            status: NodeStatus::Online,
            created_at: now,
            modified_at: now,
            label: self.label.clone(),
            interface: self.interface.clone(),
            cores: self.cores,
            max_bandwidth_bps: self.max_bandwidth_bps,
        }
    }
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
        if !self.proto.is_shadowsocks() && self.password.is_some() {
            return Err("Password only allowed for Shadowsocks".into());
        }
        if !self.proto.is_hysteria2() && self.token.is_some() {
            return Err("Token only allowed for Hysteria2".into());
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct KeyReq {
    pub days: i16,
    pub distributor: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ActivateKeyReq {
    pub code: String,
    pub subscription_id: uuid::Uuid,
}
