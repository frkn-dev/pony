use std::fmt;
use std::str::FromStr;
use std::{collections::HashMap, net::Ipv4Addr};

use chrono::DateTime;
use chrono::Utc;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};

use super::tag::Tag;
use crate::config::settings::NodeConfig;
use crate::config::wireguard::WireguardSettings;
use crate::config::xray::{Config as XrayConfig, Inbound};
use crate::http::requests::NodeResponse;

#[derive(Clone, Debug, Deserialize, Serialize, Copy, ToSql, FromSql)]
#[postgres(name = "node_status", rename_all = "snake_case")]
pub enum Status {
    Online,
    Offline,
}

impl PartialEq for Status {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Status::Online, Status::Online) => true,
            (Status::Offline, Status::Offline) => true,
            _ => false,
        }
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub uuid: uuid::Uuid,
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub status: Status,
    pub label: String,
    pub interface: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub inbounds: HashMap<Tag, Inbound>,
}

impl Node {
    pub fn new(
        settings: NodeConfig,
        xray_config: Option<XrayConfig>,
        wg_config: Option<WireguardSettings>,
    ) -> Self {
        let now = Utc::now();
        let mut inbounds: HashMap<Tag, Inbound> = HashMap::new();

        let _ = {
            if let Some(config) = xray_config {
                let xray_inbounds = config
                    .inbounds
                    .into_iter()
                    .map(|inbound| (inbound.tag.clone(), inbound))
                    .collect::<HashMap<Tag, Inbound>>();

                inbounds.extend(xray_inbounds);
            }

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
                    },
                );
            }
        };

        Self {
            uuid: settings.uuid,
            env: settings.env,
            hostname: settings.hostname.expect("hostname"),
            status: Status::Online,
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
            label: self.label.clone(),
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
}

pub struct Stat {
    pub downlink: i64,
    pub uplink: i64,
    pub conn_count: i64,
}
