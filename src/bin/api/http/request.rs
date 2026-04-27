use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use pony::{Env, Error, Inbound, Node, NodeStatus, NodeType, Result, Tag, WgParam};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TagReq {
    Xray,
    Wireguard,
    Hysteria2,
    Mtproto,
}

impl TagReq {
    pub fn tags(&self) -> Vec<Tag> {
        match self {
            TagReq::Xray => vec![
                Tag::VlessTcpReality,
                Tag::VlessGrpcReality,
                Tag::VlessXhttpReality,
                Tag::Vmess,
                Tag::Shadowsocks,
            ],
            TagReq::Wireguard => vec![Tag::Wireguard],
            TagReq::Hysteria2 => vec![Tag::Hysteria2],
            TagReq::Mtproto => vec![Tag::Mtproto],
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Subscription {
    pub referred_by: Option<String>,
    pub refer_code: Option<String>,
    pub days: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub env: Env,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: uuid::Uuid,
    pub label: String,
    pub interface: String,
    pub cores: usize,
    pub max_bandwidth_bps: i64,
    pub country: String,
    pub r#type: Option<NodeType>,
}

impl NodeRequest {
    pub fn as_node(&self) -> Node {
        let now = Utc::now();

        let t = if let Some(t) = self.r#type {
            t
        } else {
            NodeType::Common
        };
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
            country: self.country.clone(),
            r#type: t,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnCreateRequest {
    pub env: Env,
    pub token: Option<uuid::Uuid>,
    pub password: Option<String>,
    pub secret: Option<String>,
    pub subscription_id: Option<uuid::Uuid>,
    pub proto: Tag,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
    pub days: Option<u16>,
}

impl ConnCreateRequest {
    pub fn validate(&self) -> Result<()> {
        if self.password.is_some() && self.wg.is_some() {
            return Err(Error::Custom("Cannot specify both password and wg".into()));
        }
        if self.token.is_some() && self.wg.is_some() {
            return Err(Error::Custom("Cannot specify both token and wg".into()));
        }

        if self.secret.is_some() && self.wg.is_some() {
            return Err(Error::Custom("Cannot specify both secret and wg".into()));
        }

        if self.token.is_some() && self.secret.is_some() {
            return Err(Error::Custom("Cannot specify both token and secret".into()));
        }

        if self.secret.is_some() && self.password.is_some() {
            return Err(Error::Custom(
                "Cannot specify both secret and password".into(),
            ));
        }
        if self.token.is_some() && self.password.is_some() {
            return Err(Error::Custom(
                "Cannot specify both token and password".into(),
            ));
        }
        if !self.proto.is_wireguard() && self.wg.is_some() {
            return Err(Error::Custom("Wg params only allowed for Wireguard".into()));
        }

        if !self.proto.is_wireguard() && self.node_id.is_some() {
            return Err(Error::Custom("node_id only allowed for Wireguard".into()));
        }
        if !self.proto.is_shadowsocks() && self.password.is_some() {
            return Err(Error::Custom(
                "Password only allowed for Shadowsocks".into(),
            ));
        }
        if !self.proto.is_hysteria2() && self.token.is_some() {
            return Err(Error::Custom("Token only allowed for Hysteria2".into()));
        }

        if !self.proto.is_mtproto() && self.secret.is_some() {
            return Err(Error::Custom("Secret only allowed for Mtproto".into()));
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
