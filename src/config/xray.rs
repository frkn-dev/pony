use crate::http::requests::InboundResponse;
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io::Read};

use crate::state::InboundStat;
use crate::state::Tag;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StreamSettings {
    #[serde(rename = "tcpSettings")]
    pub tcp_settings: Option<TcpSettings>,
    #[serde(rename = "realitySettings")]
    pub reality_settings: Option<RealitySettings>,
    #[serde(rename = "grpcSettings")]
    pub grpc_settings: Option<GrpcSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GrpcSettings {
    #[serde(rename = "serviceName")]
    pub service_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RealitySettings {
    #[serde(rename = "serverNames")]
    pub server_names: Vec<String>,
    #[serde(rename = "privateKey")]
    pub private_key: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(rename = "shortIds")]
    pub short_ids: Vec<String>,
    pub dest: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TcpSettings {
    pub header: Option<TcpHeader>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TcpHeader {
    pub r#type: String,
    pub request: Option<TcpRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TcpRequest {
    pub method: String,
    pub path: Vec<String>,
    pub headers: Option<std::collections::HashMap<String, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Inbound {
    pub tag: Tag,
    pub port: u16,
    #[serde(rename = "streamSettings")]
    pub stream_settings: Option<StreamSettings>,
    uplink: Option<i64>,
    downlink: Option<i64>,
    conn_count: Option<i64>,
}

impl Inbound {
    pub fn as_inbound_response(&self) -> InboundResponse {
        InboundResponse {
            port: self.port,
            stream_settings: self.stream_settings.clone(),
        }
    }

    pub fn as_inbound_stat(&self) -> InboundStat {
        InboundStat {
            uplink: self.uplink.unwrap_or(0),
            downlink: self.downlink.unwrap_or(0),
            conn_count: self.conn_count.unwrap_or(0),
        }
    }

    pub fn update_uplink(&mut self, new_uplink: i64) {
        self.uplink = Some(new_uplink);
    }

    pub fn update_downlink(&mut self, new_downlink: i64) {
        self.downlink = Some(new_downlink);
    }
    pub fn update_conn_count(&mut self, new_conn_count: i64) {
        self.conn_count = Some(new_conn_count);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Api {
    pub listen: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub inbounds: Vec<Inbound>,
    pub api: Api,
}

impl Config {
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        // ToDo validate xray config
        Ok(())
    }

    pub fn new(file_path: &str) -> Result<Config, Box<dyn std::error::Error>> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        let config: Config = serde_json::from_str(&contents)?;

        Ok(config)
    }
}
