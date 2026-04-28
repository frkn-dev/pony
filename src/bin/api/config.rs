use serde::Deserialize;
use std::net::Ipv4Addr;

use pony::{Error, IpAddrMask, Result};
use pony::{LoggingConfig, Settings};

#[derive(Clone, Debug, Deserialize, Default)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub db: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct MetricsRxConfig {
    pub reciever: String,
    pub topic: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ZmqPublisherConfig {
    pub endpoint: String,
}

impl ZmqPublisherConfig {
    pub fn validate(self) -> Result<()> {
        if !self.endpoint.starts_with("tcp://") {
            return Err(Error::Custom(
                "ZMQ endpoint should start with tcp://".into(),
            ));
        }
        Ok(())
    }
}

fn default_base_url() -> String {
    "http://localhost:8000".to_string()
}

fn default_wg_network() -> IpAddrMask {
    "10.0.0.0/8".parse().unwrap()
}

fn default_listen_address() -> Ipv4Addr {
    "127.0.0.1".parse().unwrap()
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApiServiceConfig {
    #[serde(default = "default_listen_address")]
    pub listen: Ipv4Addr,
    pub port: u16,
    pub token: String,
    pub db_sync_interval_sec: u64,
    pub subscription_restore_interval: u64,
    pub subscription_expire_interval: u64,
    pub key_sign_token: Vec<u8>,
    pub bonus_days: i64,
    pub system_refer_codes: Vec<String>,
    pub max_points: usize,
    pub retention_seconds: i64,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_wg_network")]
    pub wireguard_network: IpAddrMask,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApiSettings {
    pub api: ApiServiceConfig,
    pub logging: LoggingConfig,
    pub zmq: ZmqPublisherConfig,
    pub pg: PostgresConfig,
    pub metrics: MetricsRxConfig,
}

impl Settings for ApiSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}
