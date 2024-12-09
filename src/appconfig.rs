use serde::Deserialize;
use std::fs;

fn default_enabled() -> bool {
    true
}

fn default_env() -> String {
    "dev".to_string()
}

fn default_hostname() -> String {
    "localhost".to_string()
}

fn default_interface() -> String {
    "ens3".to_string()
}

fn default_metrics_delay() -> u64 {
    1
}

fn default_xray_daily_limit_mb() -> i64 {
    1000
}

fn default_carbon_server() -> String {
    "localhost:2003".to_string()
}

fn default_xray_api_endpoint() -> String {
    "http://localhost:23456".to_string()
}

fn default_loglevel() -> String {
    "debug".to_string()
}

fn default_logfile() -> String {
    "pony.log".to_string()
}

fn default_zmq_endpoint() -> String {
    "tcp://localhost:3000".to_string()
}

fn default_zmq_topic() -> String {
    "dev".to_string()
}

fn default_file_state() -> String {
    "users.json".to_string()
}

fn default_xray_config_path() -> String {
    "xray-config.json".to_string()
}

pub fn read_config(config_file: &str) -> Result<Settings, Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string(config_file)?;
    let settings: Settings = toml::from_str(&config_str)?;
    Ok(settings)
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct CarbonConfig {
    #[serde(default = "default_carbon_server")]
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct LoggingConfig {
    #[serde(default = "default_loglevel")]
    pub level: String,
    #[serde(default = "default_logfile")]
    pub file: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default = "default_env")]
    pub env: String,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default = "default_interface")]
    pub iface: String,
    #[serde(default = "default_metrics_delay")]
    pub metrics_delay: u64,
    #[serde(default = "default_enabled")]
    pub metrics_mode: bool,
    #[serde(default = "default_enabled")]
    pub xray_api_mode: bool,
    #[serde(default = "default_file_state")]
    pub file_state: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct XrayConfig {
    #[serde(default = "default_xray_api_endpoint")]
    pub xray_api_endpoint: String,
    #[serde(default = "default_xray_daily_limit_mb")]
    pub xray_daily_limit_mb: i64,
    #[serde(default = "default_xray_config_path")]
    pub xray_config_path: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ZmqConfig {
    #[serde(default = "default_zmq_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_zmq_topic")]
    pub topic: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub carbon: CarbonConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub xray: XrayConfig,
    #[serde(default)]
    pub zmq: ZmqConfig,
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.zmq.endpoint.starts_with("tcp://") {
            return Err("ZMQ endpoint should starts with tcp://".into());
        }
        Ok(())
    }
}
