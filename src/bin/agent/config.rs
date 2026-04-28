use serde::Deserialize;

use pony::Result;

use pony::{
    ApiAccessConfig, LoggingConfig, MetricsTxConfig, MtprotoConfig, NodeConfigRaw, Settings,
    ZmqSubscriberConfig,
};

fn default_disabled() -> bool {
    false
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct WgConfig {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    pub path: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct H2Config {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct XrayConfig {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    pub xray_config_path: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default = "default_disabled")]
    pub local: bool,
    pub snapshot_interval: u64,
    pub snapshot_path: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AgentSettings {
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub xray: XrayConfig,
    #[serde(default)]
    pub wg: WgConfig,
    #[serde(default)]
    pub h2: H2Config,
    #[serde(default)]
    pub mtproto: MtprotoConfig,
    #[serde(default)]
    pub zmq: ZmqSubscriberConfig,
    #[serde(default)]
    pub node: NodeConfigRaw,
    #[serde(default)]
    pub api: ApiAccessConfig,
    #[serde(default)]
    pub metrics: MetricsTxConfig,
}

impl Settings for AgentSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}
