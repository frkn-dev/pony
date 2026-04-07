use crate::PonyError;
use crate::Result;
use default_net::{get_default_interface, get_interfaces};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use std::env;
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
fn default_carbon_server() -> String {
    "localhost:2003".to_string()
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
fn default_metrics_zmq_pub_endpoint() -> String {
    "tcp://127.0.0.1:5555".to_string()
}
fn default_xray_config_path() -> String {
    "dev/xray-config.json".to_string()
}
fn default_wg_port() -> u16 {
    51820
}
fn default_wg_interface() -> String {
    "wg0".to_string()
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
fn default_auth_web_server() -> Option<Ipv4Addr> {
    Some(Ipv4Addr::new(127, 0, 0, 1))
}
fn default_auth_web_port() -> u16 {
    3005
}
fn default_api_web_listen() -> Option<Ipv4Addr> {
    Some(Ipv4Addr::new(127, 0, 0, 1))
}
fn default_api_web_port() -> u16 {
    3005
}
fn default_api_token() -> String {
    "token".to_string()
}

fn default_key_sign_token() -> Vec<u8> {
    b"sign-token".to_vec()
}

fn default_label() -> String {
    "🏴‍☠️🏴‍☠️🏴‍☠️ dev".to_string()
}

fn default_stat_job_interval() -> u64 {
    60
}

fn default_snapshot_interval() -> u64 {
    30
}

fn default_snapshot_path() -> String {
    "snapshots/snapshot.bin".to_string()
}

fn default_metrics_interval() -> u64 {
    60
}

fn default_metrics_hb_interval() -> u64 {
    1
}

fn default_healthcheck_interval() -> u64 {
    60
}
fn default_collect_conn_stat_interval() -> u64 {
    60
}
fn default_db_sync_interval_sec() -> u64 {
    300
}

fn default_subscription_restore_interval_sec() -> u64 {
    60
}

fn default_subscription_expire_interval_sec() -> u64 {
    60
}

fn default_node_healthcheck_timeout() -> i16 {
    60
}

fn default_max_bandwidth_bps() -> i64 {
    100_000_000
}

fn default_web_host() -> String {
    "http://localhost:8000".to_string()
}

fn default_api_web_host() -> String {
    "http://localhost:5005".to_string()
}

fn default_h2_config_path() -> String {
    "dev/h2.yaml".to_string()
}

fn default_mtproto_port() -> u16 {
    8443
}

fn default_smtp_username() -> String {
    "user".to_string()
}

fn default_smtp_password() -> String {
    "password".to_string()
}

fn default_smtp_server() -> String {
    "smtp.example.com".to_string()
}

fn default_smtp_port() -> u16 {
    587
}

fn default_smtp_from() -> String {
    "noreply@example.com".to_string()
}

fn default_email_file() -> String {
    "trials.csv".to_string()
}

fn default_email_sign_token() -> Vec<u8> {
    b"email-sign-token".to_vec()
}

fn default_bonus_days() -> i64 {
    7
}

fn default_promo_codes() -> Vec<String> {
    vec![
        "FRKN.ORG".to_string(),
        "mobile".to_string(),
        "mobile-dev".to_string(),
    ]
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiServiceConfig {
    #[serde(default = "default_api_web_listen")]
    pub address: Option<Ipv4Addr>,
    #[serde(default = "default_api_web_port")]
    pub port: u16,
    #[serde(default = "default_collect_conn_stat_interval")]
    pub collect_conn_stat_interval: u64,
    #[serde(default = "default_healthcheck_interval")]
    pub healthcheck_interval: u64,
    #[serde(default = "default_api_token")]
    pub token: String,
    #[serde(default = "default_enabled")]
    pub metrics_enabled: bool,
    #[serde(default = "default_metrics_interval")]
    pub metrics_interval: u64,
    #[serde(default = "default_metrics_hb_interval")]
    pub metrics_hb_interval: u64,
    #[serde(default = "default_db_sync_interval_sec")]
    pub db_sync_interval_sec: u64,
    #[serde(default = "default_web_host")]
    pub web_host: String,
    #[serde(default = "default_api_web_host")]
    pub api_web_host: String,
    #[serde(default = "default_subscription_restore_interval_sec")]
    pub subscription_restore_interval: u64,
    #[serde(default = "default_subscription_expire_interval_sec")]
    pub subscription_expire_interval: u64,
    #[serde(default = "default_node_healthcheck_timeout")]
    pub node_health_check_timeout: i16,
    #[serde(default = "default_key_sign_token")]
    pub key_sign_token: Vec<u8>,
    #[serde(default = "default_bonus_days")]
    pub bonus_days: i64,
    #[serde(default = "default_promo_codes")]
    pub promo_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiAccessConfig {
    #[serde(default = "default_api_endpoint_address")]
    pub endpoint: String,
    #[serde(default = "default_api_token")]
    pub token: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AuthServiceConfig {
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval: u64,
    #[serde(default = "default_snapshot_path")]
    pub snapshot_path: String,
    #[serde(default = "default_web_host")]
    pub web_host: String,
    #[serde(default = "default_auth_web_server")]
    pub web_server: Option<Ipv4Addr>,
    #[serde(default = "default_auth_web_port")]
    pub web_port: u16,
    #[serde(default = "default_email_file")]
    pub email_file: String,
    #[serde(default = "default_email_sign_token")]
    pub email_sign_token: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SmtpConfig {
    #[serde(default = "default_smtp_server")]
    pub server: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default = "default_smtp_username")]
    pub username: String,
    #[serde(default = "default_smtp_password")]
    pub password: String,
    #[serde(default = "default_smtp_from")]
    pub from: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default = "default_disabled")]
    pub local: bool,
    #[serde(default = "default_enabled")]
    pub stat_enabled: bool,
    #[serde(default = "default_stat_job_interval")]
    pub stat_job_interval: u64,
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval: u64,
    #[serde(default = "default_snapshot_path")]
    pub snapshot_path: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_zmq_pub_endpoint")]
    pub publisher: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_metrics_interval")]
    pub interval: u64,
    #[serde(default = "default_metrics_hb_interval")]
    pub hb_interval: u64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct CarbonConfig {
    #[serde(default = "default_carbon_server")]
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

#[derive(Clone, Debug, Deserialize)]
pub struct NodeConfig {
    pub env: String,
    pub hostname: String,
    pub default_interface: String,
    pub address: Ipv4Addr,
    pub uuid: Uuid,
    pub label: String,
    pub max_bandwidth_bps: i64,
    pub cores: usize,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct NodeConfigRaw {
    #[serde(default = "default_env")]
    pub env: String,
    pub hostname: Option<String>,
    pub default_interface: Option<String>,
    pub address: Option<Ipv4Addr>,
    #[serde(default = "default_uuid")]
    pub uuid: Uuid,
    #[serde(default = "default_label")]
    pub label: String,
    #[serde(default = "default_max_bandwidth_bps")]
    pub max_bandwidth_bps: i64,
}

impl NodeConfig {
    pub fn from_raw(raw: NodeConfigRaw) -> Result<NodeConfig> {
        let num_cpus = std::thread::available_parallelism()?.get();
        let hostname = if raw.hostname.is_none() {
            match env::var("HOSTNAME") {
                Ok(hostname) => hostname,
                Err(_) => {
                    return Err(PonyError::Custom("Validation error: missing hostname (set $HOSTNAME env or specify in config)".into()));
                }
            }
        } else {
            raw.hostname.unwrap()
        };

        let (address, interface) = if let Some(user_address) = raw.address {
            let interface = if let Some(ref interface_name) = raw.default_interface {
                let interfaces = get_interfaces();
                if let Some(_interface) = interfaces.iter().find(|i| &i.name == interface_name) {
                    interface_name.clone()
                } else {
                    return Err(PonyError::Custom(format!(
                        "Validation error: Interface {} not found",
                        interface_name
                    )));
                }
            } else {
                match get_default_interface() {
                    Ok(interface) => interface.name,
                    Err(e) => {
                        eprintln!(
                            "Warning: Cannot get default interface: {}. Using 'default'.",
                            e
                        );
                        "default".to_string()
                    }
                }
            };

            (user_address, interface)
        } else if let Some(ref interface_name) = raw.default_interface {
            let interfaces = get_interfaces();
            if let Some(interface) = interfaces.iter().find(|i| &i.name == interface_name) {
                match interface.ipv4.first() {
                    Some(network) => (network.addr, interface_name.to_string()),
                    None => {
                        return Err(PonyError::Custom(
                            "Validation error: Cannot get IPv4 address for the specified interface"
                                .into(),
                        ));
                    }
                }
            } else {
                return Err(PonyError::Custom(format!(
                    "Validation error: Interface {} not found",
                    interface_name
                )));
            }
        } else {
            match get_default_interface() {
                Ok(interface) => {
                    if interface.ipv4.is_empty() {
                        return Err(PonyError::Custom(
                            "Validation error: Cannot get IPv4 address of default interface".into(),
                        ));
                    } else {
                        (interface.ipv4[0].addr, interface.name)
                    }
                }
                Err(e) => {
                    return Err(
                        format!("Validation error: Cannot get default interface: {}", e).into(),
                    )
                }
            }
        };

        Ok(NodeConfig {
            env: raw.env,
            hostname,
            default_interface: interface,
            address,
            uuid: raw.uuid,
            label: raw.label,
            max_bandwidth_bps: raw.max_bandwidth_bps,
            cores: num_cpus,
        })
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
pub struct XrayConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_xray_config_path")]
    pub xray_config_path: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct WgConfig {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    #[serde(default = "default_wg_port")]
    pub port: u16,
    #[serde(default = "default_wg_interface")]
    pub interface: String,
    pub network: Option<String>,
    pub privkey: Option<String>,
    pub pubkey: Option<String>,
    pub address: Option<String>,
    pub dns: Option<Vec<String>>,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct H2Config {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    #[serde(default = "default_h2_config_path")]
    pub path: String,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct MtprotoConfig {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    #[serde(default = "default_mtproto_port")]
    pub port: u16,
    pub secret: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ZmqSubscriberConfig {
    #[serde(default = "default_zmq_sub_endpoint")]
    pub endpoint: String,
}

impl ZmqSubscriberConfig {
    pub fn validate(self) -> Result<()> {
        if !self.endpoint.starts_with("tcp://") {
            return Err(PonyError::Custom(
                "ZMQ endpoint should start with tcp://".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ZmqPublisherConfig {
    #[serde(default = "default_zmq_pub_endpoint")]
    pub endpoint: String,
}

impl ZmqPublisherConfig {
    pub fn validate(self) -> Result<()> {
        if !self.endpoint.starts_with("tcp://") {
            return Err(PonyError::Custom(
                "ZMQ endpoint should start with tcp://".into(),
            ));
        }
        Ok(())
    }
}

pub trait Settings: Sized {
    fn read_config<T: DeserializeOwned>(config_file: &str) -> Result<T> {
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

    fn validate(&self) -> Result<()>;
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiSettings {
    #[serde(default)]
    pub api: ApiServiceConfig,
    #[serde(default)]
    pub debug: DebugConfig,
    #[serde(default)]
    pub node: NodeConfigRaw,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub zmq: ZmqPublisherConfig,
    #[serde(default)]
    pub pg: PostgresConfig,
    #[serde(default)]
    pub carbon: CarbonConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthServiceSettings {
    #[serde(default)]
    pub debug: DebugConfig,
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
    pub metrics: MetricsConfig,
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
    pub metrics: MetricsConfig,
}

impl Settings for AgentSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}

impl Settings for ApiSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}

impl Settings for AuthServiceSettings {
    fn validate(&self) -> Result<()> {
        self.zmq.clone().validate()?;
        Ok(())
    }
}
