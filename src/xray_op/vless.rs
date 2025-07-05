use std::fmt;
use std::net::Ipv4Addr;
use url::Url;

use crate::http::requests::InboundResponse;
use crate::xray_api::xray::proxy::vless;
use crate::xray_api::xray::{common::protocol::User, common::serial::TypedMessage};
use crate::xray_op::ProtocolConn;
use crate::xray_op::Tag;
use crate::PonyError;
use crate::Result as PonyResult;

#[derive(Clone, Debug)]
pub struct ConnInfo {
    pub uuid: uuid::Uuid,
    pub in_tag: Tag,
    pub level: u32,
    pub email: String,
    encryption: Option<String>,
    flow: ConnFlow,
}

impl ConnInfo {
    pub fn new(uuid: &uuid::Uuid, flow: ConnFlow) -> Self {
        let tag = match flow {
            ConnFlow::Vision => Tag::VlessXtls,
            ConnFlow::Direct => Tag::VlessGrpc,
        };

        Self {
            in_tag: tag,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            uuid: *uuid,
            encryption: Some("none".to_string()),
            flow: flow,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConnFlow {
    Vision,
    Direct,
}

impl fmt::Display for ConnFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnFlow::Vision => write!(f, "xtls-rprx-vision"),
            ConnFlow::Direct => write!(f, "xtls-rprx-direct"),
        }
    }
}

#[async_trait::async_trait]
impl ProtocolConn for ConnInfo {
    fn tag(&self) -> Tag {
        self.in_tag.clone()
    }
    fn email(&self) -> String {
        self.email.clone()
    }
    fn to_user(&self) -> Result<User, Box<dyn std::error::Error + Send + Sync>> {
        let account = vless::Account {
            id: self.uuid.to_string(),
            flow: self.flow.to_string(),
            encryption: self.encryption.clone().unwrap_or("none".to_string()),
        };

        Ok(User {
            level: self.level,
            email: self.email.clone(),
            account: Some(TypedMessage {
                r#type: "xray.proxy.vless.Account".to_string(),
                value: prost::Message::encode_to_vec(&account),
            }),
        })
    }
}

pub fn vless_xtls_conn(
    conn_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
) -> PonyResult<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings.ok_or(PonyError::Custom(
        "VLESS XTLS: stream settings error".into(),
    ))?;
    let reality_settings = stream_settings.reality_settings.ok_or(PonyError::Custom(
        "VLESS XTLS: reality settings error".into(),
    ))?;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first().ok_or(PonyError::Custom(
        "VLESS XTLS: reality settings SID error".into(),
    ))?;
    let sni = reality_settings
        .server_names
        .first()
        .ok_or(PonyError::Custom(
            "VLESS XTLS: reality settings SNI error".into(),
        ))?;

    let mut url = Url::parse(&format!("vless://{conn_id}@{ipv4}:{port}"))?;
    url.query_pairs_mut()
        .append_pair("security", "reality")
        .append_pair("flow", "xtls-rprx-vision")
        .append_pair("type", "tcp")
        .append_pair("sni", &sni)
        .append_pair("fp", "chrome")
        .append_pair("pbk", &pbk)
        .append_pair("sid", &sid);

    url.set_fragment(Some(&format!("{} XTLS", label)));

    Ok(url.to_string())
}

pub fn vless_grpc_conn(
    conn_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
) -> PonyResult<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings.ok_or(PonyError::Custom(
        "VLESS GRPC: stream settings error".into(),
    ))?;
    let reality_settings = stream_settings.reality_settings.ok_or(PonyError::Custom(
        "VLESS GRPC: reality settings error".into(),
    ))?;
    let grpc_settings = stream_settings
        .grpc_settings
        .ok_or(PonyError::Custom("VLESS GRPC: grpc settings error".into()))?;
    let service_name = grpc_settings.service_name;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first().ok_or(PonyError::Custom(
        "VLESS GRPC: reality settings SID error".into(),
    ))?;
    let sni = reality_settings
        .server_names
        .first()
        .ok_or(PonyError::Custom(
            "VLESS GRPC: reality settings SNI error".into(),
        ))?;

    let mut url = Url::parse(&format!("vless://{conn_id}@{ipv4}:{port}"))?;

    url.query_pairs_mut()
        .append_pair("security", "reality")
        .append_pair("type", "grpc")
        .append_pair("mode", "gun")
        .append_pair("serviceName", &service_name)
        .append_pair("fp", "chrome")
        .append_pair("sni", sni)
        .append_pair("pbk", &pbk)
        .append_pair("sid", sid);

    url.set_fragment(Some(&format!("{label} GRPC")));

    Ok(url.to_string())
}
