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
    pub from: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AuthServiceConfig {
    pub snapshot_interval: u64,
    pub snapshot_path: String,
    pub web_host: String,
    pub listen: Option<Ipv4Addr>,
    pub web_port: u16,
    pub email_file: String,
    pub email_sign_token: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthServiceSettings {
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub auth: AuthServiceConfig,
    #[serde(default)]
    pub zmq: ZmqSubscriberConfig,
    #[serde(default)]
    pub node: NodeConfigRaw,
    #[serde(default)]
    pub api: ApiAccessConfig,
    #[serde(default)]
    pub smtp: SmtpConfig,
    #[serde(default)]
    pub metrics: MetricsTxConfig,
}

impl Settings for AuthServiceSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}
