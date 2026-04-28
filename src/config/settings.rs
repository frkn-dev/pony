use default_net::{get_default_interface, get_interfaces};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::env;
use std::fs;
use std::net::Ipv4Addr;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::memory::node::Type;

fn default_disabled() -> bool {
    false
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ApiAccessConfig {
    pub endpoint: String,
    pub token: String,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct MetricsTxConfig {
    pub publisher: String,
    pub interval: u64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct LoggingConfig {
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
    pub country: String,
    pub r#type: Type,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct NodeConfigRaw {
    pub env: String,
    pub hostname: Option<String>,
    pub default_interface: Option<String>,
    pub address: Option<Ipv4Addr>,
    pub uuid: Uuid,
    pub label: String,
    pub max_bandwidth_bps: i64,
    pub country: String,
    pub r#type: String,
}

impl NodeConfig {
    pub fn from_raw(raw: NodeConfigRaw) -> Result<NodeConfig> {
        let num_cpus = std::thread::available_parallelism()?.get();
        let hostname = if raw.hostname.is_none() {
            match env::var("HOSTNAME") {
                Ok(hostname) => hostname,
                Err(_) => {
                    return Err(Error::Custom("Validation error: missing hostname (set $HOSTNAME env or specify in config)".into()));
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
                    return Err(Error::Custom(format!(
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
                        return Err(Error::Custom(
                            "Validation error: Cannot get IPv4 address for the specified interface"
                                .into(),
                        ));
                    }
                }
            } else {
                return Err(Error::Custom(format!(
                    "Validation error: Interface {} not found",
                    interface_name
                )));
            }
        } else {
            match get_default_interface() {
                Ok(interface) => {
                    if interface.ipv4.is_empty() {
                        return Err(Error::Custom(
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
            country: raw.country,
            r#type: raw.r#type.parse().unwrap_or(Type::Common),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ZmqSubscriberConfig {
    pub endpoint: String,
}

impl ZmqSubscriberConfig {
    pub fn validate(self) -> Result<()> {
        if !self.endpoint.starts_with("tcp://") {
            return Err(Error::Custom(
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

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct MtprotoConfig {
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    pub port: u16,
    pub secret: String,
}
