use default_net::{get_default_interface, get_interfaces};
use log::debug;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use uuid::Uuid;

fn default_enabled() -> bool {
    true
}

fn default_disabled() -> bool {
    false
}

fn default_env() -> String {
    "dev".to_string()
}

fn default_user_daily_limit_mb() -> i64 {
    1000
}

fn default_carbon_server() -> String {
    "localhost:2003".to_string()
}

fn default_clickhouse_server() -> String {
    "http://localhost:8123".to_string()
}

fn default_loglevel() -> String {
    "debug".to_string()
}

fn default_zmq_sub_endpoint() -> String {
    "tcp://localhost:3000".to_string()
}

fn default_zmq_pub_endpoint() -> String {
    "tcp://*:3000".to_string()
}

fn default_xray_config_path() -> String {
    "dev/xray-config.json".to_string()
}

fn default_pg_address() -> String {
    "localhost".to_string()
}

fn default_pg_port() -> u16 {
    5432
}

fn default_pg_db() -> String {
    "postgres".to_string()
}

fn default_pg_username() -> String {
    "postgres".to_string()
}

fn default_pg_password() -> String {
    "password".to_string()
}

fn default_stat_job_interval() -> u64 {
    60
}

fn default_metrics_interval() -> u64 {
    60
}

fn default_api_endpoint_address() -> String {
    "http://localhost:3005".to_string()
}

fn default_uuid() -> Uuid {
    Uuid::parse_str("9658b391-01cb-4031-a3f5-6cbdd749bcff").unwrap()
}

fn default_debug_web_server() -> Option<Ipv4Addr> {
    Some(Ipv4Addr::new(127, 0, 0, 1))
}

fn default_debug_web_port() -> u16 {
    3001
}

fn default_api_web_listen() -> Option<Ipv4Addr> {
    Some(Ipv4Addr::new(127, 0, 0, 1))
}

fn default_api_web_port() -> u16 {
    3005
}

fn default_api_token() -> String {
    "supetsecrettoken".to_string()
}

fn default_label() -> String {
    "🏴‍☠️🏴‍☠️🏴‍☠️ dev".to_string()
}

fn default_node_healthcheck_timeout() -> i16 {
    60
}

fn default_user_limit_check_interval() -> u64 {
    60
}

fn default_healthcheck_interval() -> u64 {
    60
}

fn default_tg_token() -> String {
    "".to_string()
}

fn default_shop_id() -> String {
    "".to_string()
}
fn default_secret_key() -> String {
    "".to_string()
}
fn default_price() -> String {
    "5.00".to_string()
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiConfig {
    #[serde(default = "default_api_web_listen")]
    pub address: Option<Ipv4Addr>,
    #[serde(default = "default_api_web_port")]
    pub port: u16,
    #[serde(default = "default_api_endpoint_address")]
    pub endpoint: String,
    #[serde(default = "default_node_healthcheck_timeout")]
    pub node_health_check_timeout: i16,
    #[serde(default = "default_user_limit_check_interval")]
    pub user_limit_check_timeout: u64,
    #[serde(default = "default_api_token")]
    pub token: String,
    #[serde(default = "default_user_daily_limit_mb")]
    pub user_limit_mb: i64,
    #[serde(default = "default_healthcheck_interval")]
    pub healthcheck_interval: u64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default = "default_disabled")]
    pub local: bool,
    #[serde(default = "default_enabled")]
    pub metrics_enabled: bool,
    #[serde(default = "default_metrics_interval")]
    pub metrics_interval: u64,
    #[serde(default = "default_enabled")]
    pub stat_enabled: bool,
    #[serde(default = "default_stat_job_interval")]
    pub stat_job_interval: u64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct CarbonConfig {
    #[serde(default = "default_carbon_server")]
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ClickhouseConfig {
    #[serde(default = "default_clickhouse_server")]
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct DebugConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_debug_web_server")]
    pub web_server: Option<Ipv4Addr>,
    #[serde(default = "default_debug_web_port")]
    pub web_port: u16,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct LoggingConfig {
    #[serde(default = "default_loglevel")]
    pub level: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct NodeConfig {
    #[serde(default = "default_env")]
    pub env: String,
    pub hostname: Option<String>,
    pub default_interface: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
    #[serde(default = "default_uuid")]
    pub uuid: Uuid,
    #[serde(default = "default_label")]
    pub label: String,
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

            debug!("Default interface: {}", interface_name);

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
pub struct PostgresConfig {
    #[serde(default = "default_pg_address")]
    pub host: String,
    #[serde(default = "default_pg_port")]
    pub port: u16,
    #[serde(default = "default_pg_db")]
    pub db: String,
    #[serde(default = "default_pg_username")]
    pub username: String,
    #[serde(default = "default_pg_password")]
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct YookassaConfig {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    #[serde(default = "default_shop_id")]
    pub shop_id: String,
    #[serde(default = "default_secret_key")]
    pub secret_key: String,
    #[serde(default = "default_price")]
    pub price: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct PaymentConfig {
    #[serde(default)]
    pub yookassa: YookassaConfig,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct XrayConfig {
    #[serde(default = "default_xray_config_path")]
    pub xray_config_path: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ZmqConfig {
    #[serde(default = "default_zmq_sub_endpoint")]
    pub sub_endpoint: String,
    #[serde(default = "default_zmq_pub_endpoint")]
    pub pub_endpoint: String,
}

impl ZmqConfig {
    pub fn validate(self) -> Result<(), Box<dyn Error>> {
        if !self.sub_endpoint.starts_with("tcp://") || !self.pub_endpoint.starts_with("tcp://") {
            return Err("ZMQ endpoint should start with tcp://".into());
        }
        Ok(())
    }
}

pub trait Settings: Sized {
    fn read_config<T: DeserializeOwned>(config_file: &str) -> Result<T, Box<dyn Error>> {
        let config_str = fs::read_to_string(config_file)?;
        let settings: T = toml::from_str(&config_str)?;
        Ok(settings)
    }

    fn new(config_file: &str) -> Self
    where
        for<'de> Self: Deserialize<'de>,
    {
        match Self::read_config(config_file) {
            Ok(settings) => settings,
            Err(e) => panic!("Failed to load settings: {}", e),
        }
    }

    fn validate(&mut self) -> Result<(), String>;
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiSettings {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub debug: DebugConfig,
    #[serde(default)]
    pub node: NodeConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub zmq: ZmqConfig,
    #[serde(default)]
    pub clickhouse: ClickhouseConfig,
    #[serde(default)]
    pub pg: PostgresConfig,
}

impl Settings for ApiSettings {
    fn validate(&mut self) -> Result<(), std::string::String> {
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

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AgentSettings {
    #[serde(default)]
    pub debug: DebugConfig,
    #[serde(default)]
    pub carbon: CarbonConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub xray: XrayConfig,
    #[serde(default)]
    pub zmq: ZmqConfig,
    #[serde(default)]
    pub node: NodeConfig,
    #[serde(default)]
    pub api: ApiConfig,
}

impl Settings for AgentSettings {
    fn validate(&mut self) -> Result<(), String> {
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

#[derive(Clone, Debug, Deserialize, Default)]
pub struct BotConfig {
    #[serde(default = "default_tg_token")]
    pub token: String,
    pub address: String,
    #[serde(default = "default_api_token")]
    pub webhook_token: String,
    #[serde(default = "default_api_web_listen")]
    pub bind_addr: Option<Ipv4Addr>,

    pub bind_port: u16,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct BotSettings {
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub pg: PostgresConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub bot: BotConfig,
    #[serde(default)]
    pub pay: PaymentConfig,
}

impl Settings for BotSettings {
    fn validate(&mut self) -> Result<(), String> {
        Ok(()) //ToDo implement validate
    }
}
