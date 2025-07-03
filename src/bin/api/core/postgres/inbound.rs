use defguard_wireguard_rs::net::IpAddrMask;
use serde::Deserialize;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::PgClient;

use pony::config::wireguard::WireguardSettings;
use pony::config::xray::Inbound;
use pony::config::xray::StreamSettings;
use pony::Result;
use pony::Tag;

pub struct PgInbound {
    pub client: Arc<Mutex<PgClient>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InboundDb {
    pub id: uuid::Uuid,
    pub node_id: uuid::Uuid,
    pub tag: Tag,
    pub port: u16,
    pub stream_settings: Option<serde_json::Value>,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub conn_count: Option<i64>,
    pub wg_pubkey: Option<String>,
    pub wg_privkey: Option<String>,
    pub wg_interface: Option<String>,
    pub wg_network: Option<IpAddrMask>,
    pub wg_address: Option<Ipv4Addr>,
}

impl InboundDb {
    pub fn as_inbound(&self) -> Result<Inbound> {
        let stream_settings: Option<StreamSettings> = match &self.stream_settings {
            Some(value) => Some(serde_json::from_value(value.clone())?),
            None => None,
        };

        let wg =
            if let (Some(pubkey), Some(privkey), Some(interface), Some(network), Some(address)) = (
                self.wg_pubkey.as_ref(),
                self.wg_privkey.as_ref(),
                self.wg_interface.as_ref(),
                self.wg_network.clone(),
                self.wg_address,
            ) {
                Some(WireguardSettings {
                    pubkey: pubkey.clone(),
                    privkey: privkey.clone(),
                    interface: interface.clone(),
                    network,
                    port: self.port,
                    address: address,
                })
            } else {
                None
            };

        Ok(Inbound {
            tag: self.tag,
            port: self.port,
            stream_settings,
            uplink: self.uplink,
            downlink: self.downlink,
            conn_count: self.conn_count,
            wg,
        })
    }
}

impl PgInbound {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }
}
