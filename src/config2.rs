use serde::Deserialize;
use std::fs;

fn default_enabled() -> bool {
    false
}

fn default_vmess_port() -> u16 {
    10086
}

fn default_vless_port() -> u16 {
    10087
}

fn default_ss_port() -> u16 {
    10088
}

fn default_wg_port() -> u16 {
    51820
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

fn default_carbon_server() -> String {
    "localhost:2003".to_string()
}

fn default_ch_server() -> String {
    "http://localhost:8123".to_string()
}

fn default_loglevel() -> String {
    "debug".to_string()
}

fn default_logfile() -> String {
    "pony.log".to_string()
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
pub struct ChConfig {
    #[serde(default = "default_ch_server")]
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
    pub api_mode: bool,
    #[serde(default = "default_enabled")]
    pub metrics_mode: bool,
    pub api_token: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct XrayConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_vmess_port")]
    pub vmess_port: u16,
    #[serde(default = "default_vless_port")]
    pub vless_port: u16,
    #[serde(default = "default_ss_port")]
    pub ss_port: u16,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct WgConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_wg_port")]
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub carbon: CarbonConfig,
    pub clickhouse: ChConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub xray: XrayConfig,
    #[serde(default)]
    pub wg: WgConfig,
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.xray.enabled && self.xray.vmess_port == 0 {
            return Err("Xray vmess port coulnd't be 0".into());
        }
        if self.xray.enabled && self.xray.vless_port == 0 {
            return Err("Xray vless port coulnd't be 0".into());
        }
        if self.xray.enabled && self.xray.ss_port == 0 {
            return Err("Xray ss port coulnd't be 0".into());
        }
        if self.wg.enabled && self.wg.port == 0 {
            return Err("Xray wg порт port coulnd't be 0".into());
        }
        // Другие проверки
        Ok(())
    }
}
