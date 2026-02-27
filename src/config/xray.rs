use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read};

use crate::config::h2::H2Settings;
use crate::config::wireguard::WireguardSettings;
use crate::http::requests::InboundResponse;
use crate::memory::node::Stat as InboundStat;
use crate::memory::tag::ProtoTag as Tag;
use crate::Result;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Xhttp,
    Grpc,
    Tcp,
    Ws,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StreamSettings {
    pub network: Network,
    #[serde(rename = "tcpSettings")]
    pub tcp_settings: Option<TcpSettings>,
    #[serde(rename = "realitySettings")]
    pub reality_settings: Option<RealitySettings>,
    #[serde(rename = "grpcSettings")]
    pub grpc_settings: Option<GrpcSettings>,
    #[serde(rename = "xhttpSettings")]
    pub xhttp_settings: Option<XhttpSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GrpcSettings {
    #[serde(rename = "serviceName")]
    pub service_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RealitySettings {
    #[serde(rename = "serverNames")]
    pub server_names: Vec<String>,
    #[serde(rename = "privateKey")]
    pub private_key: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(rename = "shortIds")]
    pub short_ids: Vec<String>,
    pub target: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct XhttpSettings {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TcpSettings {
    pub header: Option<TcpHeader>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TcpHeader {
    pub r#type: String,
    pub request: Option<TcpRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TcpRequest {
    pub method: String,
    pub path: Vec<String>,
    pub headers: Option<std::collections::HashMap<String, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Inbound {
    pub tag: Tag,
    pub port: u16,
    #[serde(rename = "streamSettings")]
    pub stream_settings: Option<StreamSettings>,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub conn_count: Option<i64>,
    pub wg: Option<WireguardSettings>,
    pub h2: Option<H2Settings>,
    pub mtproto_secret: Option<String>,
}

impl Inbound {
    pub fn as_inbound_response(&self) -> InboundResponse {
        InboundResponse {
            port: self.port,
            stream_settings: self.stream_settings.clone(),
            tag: self.tag,
            wg: self.wg.clone(),
            h2: self.h2.clone(),
            mtproto_secret: self.mtproto_secret.clone(),
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
    pub fn validate(&self) -> Result<()> {
        // ToDo validate xray config
        Ok(())
    }

    pub fn new(file_path: &str) -> Result<Config> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        let config: Config = serde_json::from_str(&contents)?;

        Ok(config)
    }
}
