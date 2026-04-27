use serde::Deserialize;
use std::net::Ipv4Addr;

use pony::{Error, Result};
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

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiServiceConfig {
    pub listen: Option<Ipv4Addr>,
    pub port: u16,
    pub token: String,
    pub db_sync_interval_sec: u64,
    pub subscription_restore_interval: u64,
    pub subscription_expire_interval: u64,
    pub key_sign_token: Vec<u8>,
    pub bonus_days: i64,
    pub promo_codes: Vec<String>,
    pub max_points: usize,
    pub retention_seconds: i64,
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiSettings {
    #[serde(default)]
    pub api: ApiServiceConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub zmq: ZmqPublisherConfig,
    #[serde(default)]
    pub pg: PostgresConfig,
    #[serde(default)]
    pub metrics: MetricsRxConfig,
}

impl Settings for ApiSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}
