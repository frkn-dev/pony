use serde::Deserialize;
use std::net::Ipv4Addr;

use pony::Result;
use pony::{
    ApiAccessConfig, LoggingConfig, MetricsTxConfig, NodeConfigRaw, Settings, ZmqSubscriberConfig,
};

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SmtpConfig {
    pub server: String,
    pub username: String,
    pub password: String,
    pub port: u16,
    pub from: String,
}

fn default_listen_address() -> Ipv4Addr {
    "127.0.0.1".parse().unwrap()
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthServiceConfig {
    pub snapshot_interval: u64,
    pub snapshot_path: String,
    pub web_host: String,
    #[serde(default = "default_listen_address")]
    pub listen: Ipv4Addr,
    pub port: u16,
    pub email_file: String,
    pub email_sign_token: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthServiceSettings {
    pub logging: LoggingConfig,
    pub auth: AuthServiceConfig,
    pub zmq: ZmqSubscriberConfig,
    pub node: NodeConfigRaw,
    pub api: ApiAccessConfig,
    pub smtp: SmtpConfig,
    pub metrics: MetricsTxConfig,
}

impl Settings for AuthServiceSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}
