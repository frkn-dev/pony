use std::fmt;
use std::str::FromStr;
use std::{
    collections::{BTreeMap, HashMap},
    net::Ipv4Addr,
};

use chrono::DateTime;
use chrono::Utc;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};

use super::env::Env;
use super::tag::ProtoTag as Tag;
use crate::config::h2::H2Settings;

#[cfg(feature = "xray")]
use crate::config::inbound::Settings as XraySettings;

use crate::config::inbound::Inbound;
use crate::config::mtproto::MtprotoSettings;
use crate::config::settings::NodeConfig;
#[cfg(feature = "wireguard")]
use crate::config::wireguard::WireguardSettings;

#[derive(Clone, Debug, Deserialize, Serialize, Copy, ToSql, FromSql)]
#[postgres(name = "node_status", rename_all = "snake_case")]
pub enum Status {
    Online,
    Offline,
}

impl PartialEq for Status {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Status::Online, Status::Online) | (Status::Offline, Status::Offline)
        )
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Online => write!(f, "Online"),
            Status::Offline => write!(f, "Offline"),
        }
    }
}

impl FromStr for Status {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Online" => Ok(Status::Online),
            "Offline" => Ok(Status::Offline),
            _ => Ok(Status::Offline),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Copy, ToSql, FromSql, PartialEq)]
#[postgres(name = "node_type", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Common,
    Premium,
    Service,
    Agent,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Common => write!(f, "Common"),
            Type::Premium => write!(f, "Premium"),
            Type::Service => write!(f, "Service"),
            Type::Agent => write!(f, "Agent"),
        }
    }
}

impl FromStr for Type {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Common" => Ok(Type::Common),
            "Premium" => Ok(Type::Premium),
            "Service" => Ok(Type::Service),
            "Agent" => Ok(Type::Agent),
            _ => Ok(Type::Common),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeResponse {
    pub uuid: uuid::Uuid,
    pub env: String,
    pub hostname: String,
    pub interface: String,
    pub address: Ipv4Addr,
    pub inbounds: Vec<Tag>,
    pub status: Status,
    pub label: String,
    pub cores: usize,
    pub max_bandwidth_bps: i64,
    pub metrics: Vec<NodeMetricInfo>,
    pub country: String,
    pub r#type: Type,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NodeMetricInfo {
    pub key: String,
    pub name: String,
    pub tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Node {
    pub uuid: uuid::Uuid,
    pub env: Env,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub status: Status,
    pub label: String,
    pub interface: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub inbounds: HashMap<Tag, Inbound>,
    pub cores: usize,
    pub max_bandwidth_bps: i64,
    pub country: String,
    pub r#type: Type,
}

impl Node {
    pub fn new(
        settings: NodeConfig,
        #[cfg(feature = "xray")] xray_config: Option<XraySettings>,
        #[cfg(feature = "wireguard")] wg_config: Option<WireguardSettings>,
        h2_config: Option<H2Settings>,
        mtproto_config: Option<MtprotoSettings>,
    ) -> Self {
        let now = Utc::now();
        let mut inbounds: HashMap<Tag, Inbound> = HashMap::new();

        {
            #[cfg(feature = "xray")]
            if let Some(config) = xray_config {
                let xray_inbounds = config
                    .inbounds
                    .into_iter()
                    .map(|inbound| (inbound.tag, inbound))
                    .collect::<HashMap<Tag, Inbound>>();

                inbounds.extend(xray_inbounds);
            }

            #[cfg(feature = "wireguard")]
            if let Some(ref config) = wg_config {
                inbounds.insert(
                    Tag::Wireguard,
                    Inbound {
                        port: config.port,
                        tag: Tag::Wireguard,
                        stream_settings: None,
                        uplink: None,
                        downlink: None,
                        conn_count: None,
                        wg: wg_config,
                        h2: None,
                        mtproto_secret: None,
                    },
                );
            }

            if let Some(ref config) = mtproto_config {
                inbounds.insert(
                    Tag::Mtproto,
                    Inbound {
                        port: config.port,
                        tag: Tag::Mtproto,
                        stream_settings: None,
                        uplink: None,
                        downlink: None,
                        conn_count: None,
                        wg: None,
                        h2: None,
                        mtproto_secret: Some(config.secret.clone()),
                    },
                );
            }

            if let Some(ref config) = h2_config {
                inbounds.insert(
                    Tag::Hysteria2,
                    Inbound {
                        port: config.port,
                        tag: Tag::Hysteria2,
                        stream_settings: None,
                        uplink: None,
                        downlink: None,
                        conn_count: None,
                        wg: None,
                        h2: h2_config,
                        mtproto_secret: None,
                    },
                );
            }
        };
        Self {
            uuid: settings.uuid,
            env: settings.env.into(),
            hostname: settings.hostname,
            status: Status::Online,
            address: settings.address,
            created_at: now,
            label: settings.label,
            interface: settings.default_interface,
            modified_at: now,
            inbounds,
            cores: settings.cores,
            max_bandwidth_bps: settings.max_bandwidth_bps,
            country: settings.country,
            r#type: settings.r#type,
        }
    }

    pub fn get_base_tags(&self) -> BTreeMap<String, String> {
        let mut tags = BTreeMap::new();
        tags.insert("env".to_string(), self.env.to_string());
        tags.insert("hostname".to_string(), self.hostname.clone());
        tags.insert("label".to_string(), self.label.clone());
        tags.insert("address".to_string(), self.address.to_string());
        tags.insert("label".to_string(), self.label.clone());
        tags.insert("cores".to_string(), self.cores.to_string());
        tags.insert(
            "max_bandwidth_bps".to_string(),
            self.max_bandwidth_bps.to_string(),
        );
        tags.insert("country".to_string(), self.country.clone());
        tags.insert("type".to_string(), self.r#type.to_string());
        tags
    }

    pub fn as_node_response(&self) -> NodeResponse {
        let tags: Vec<Tag> = self.inbounds.keys().cloned().collect();

        NodeResponse {
            env: self.env.to_string(),
            hostname: self.hostname.clone(),
            interface: self.interface.clone(),
            address: self.address,
            uuid: self.uuid,
            inbounds: tags,
            status: self.status,
            label: self.label.clone(),
            cores: self.cores,
            max_bandwidth_bps: self.max_bandwidth_bps,
            metrics: [].to_vec(),
            country: self.country.clone(),
            r#type: self.r#type,
        }
    }

    pub fn update_status(&mut self, new_status: Status) -> Result<(), String> {
        self.status = new_status;
        Ok(())
    }

    pub fn update_uplink(&mut self, tag: &Tag, new_uplink: i64) -> Result<(), String> {
        if let Some(inbound) = self.inbounds.get_mut(tag) {
            inbound.update_uplink(new_uplink);
            Ok(())
        } else {
            Err(format!("Inbound {}  not found", tag))
        }
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

    pub fn inbound(&self, tag: Tag) -> Option<&Inbound> {
        self.inbounds.values().find(|i| i.tag == tag)
    }
}

pub struct Stat {
    pub downlink: i64,
    pub uplink: i64,
    pub conn_count: i64,
}
