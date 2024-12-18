use default_net::{get_default_interface, get_interfaces};
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;

fn default_enabled() -> bool {
    true
}

fn default_env() -> String {
    "dev".to_string()
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
pub struct NodeConfig {
    #[serde(default = "default_env")]
    pub env: String,
    pub hostname: Option<String>,
    pub default_interface: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
}

impl NodeConfig {
    pub fn validate(&mut self) -> Result<(), Box<dyn Error>> {
        if self.hostname.is_none() {
            match env::var("HOSTNAME") {
                Ok(hostname) => {
                    self.hostname = Some(hostname);
                }
                Err(_) => {
                    return Err("Error -> Define $HOSTNAME env or hostname in config".into());
                }
            }
        }

        if self.default_interface.is_none() && self.ipv4.is_none() {
            match get_default_interface() {
                Ok(interface) => {
                    self.default_interface = Some(interface.name);
                    if interface.ipv4.is_empty() {
                        return Err("Cannot get ipv4 addr of interface: {}".into());
                    } else {
                        self.ipv4 = Some(interface.ipv4[0].addr);
                    }
                }
                Err(e) => return Err(format!("Cannot get default interface: {}", e).into()),
            }
        } else {
            let interface_name = &self.default_interface.clone().expect("interface");
            let interfaces = get_interfaces();
            let interface = interfaces
                .iter()
                .find(|interface| &interface.name == interface_name);

            if let Some(interface) = interface.clone() {
                match interface.ipv4.first() {
                    Some(network) => self.ipv4 = Some(network.addr),
                    None => return Err("Cannot get interface addr".into()),
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default = "default_metrics_delay")]
    pub metrics_delay: u64,
    #[serde(default = "default_file_state")]
    pub file_state: String,
    #[serde(default = "default_enabled")]
    pub debug: bool,
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

impl ZmqConfig {
    pub fn validate(self) -> Result<(), Box<dyn Error>> {
        if !self.endpoint.starts_with("tcp://") {
            return Err("ZMQ endpoint should start with tcp://".into());
        }
        Ok(())
    }
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
    #[serde(default)]
    pub node: NodeConfig,
}

impl Settings {
    pub fn validate(&mut self) -> Result<(), String> {
        self.node
            .validate()
            .map_err(|e| format!("Node configuration validation error: {}", e))?;

        self.zmq
            .clone()
            .validate()
            .map_err(|e| format!("Zmq configuration validation error: {}", e))?;

        Ok(())
    }
}
