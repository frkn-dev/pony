use base64::Engine;
use serde::Serialize;
use std::net::Ipv4Addr;

use super::ProtocolConn;
use crate::error::{PonyError, Result as PonyResult};
use crate::http::requests::InboundResponse;
use crate::memory::tag::Tag;
use crate::xray_api::xray::proxy::vmess;
use crate::xray_api::xray::{common::protocol::User, common::serial::TypedMessage};

#[derive(Clone, Debug)]
pub struct ConnInfo {
    pub in_tag: Tag,
    pub level: u32,
    pub email: String,
    pub uuid: uuid::Uuid,
}

impl ConnInfo {
    pub fn new(uuid: &uuid::Uuid) -> Self {
        Self {
            in_tag: Tag::Vmess,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            uuid: *uuid,
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
        let account = vmess::Account {
            id: self.uuid.to_string(),
            ..Default::default()
        };

        Ok(User {
            level: self.level,
            email: self.email.clone(),
            account: Some(TypedMessage {
                r#type: "xray.proxy.vmess.Account".to_string(),
                value: prost::Message::encode_to_vec(&account),
            }),
        })
    }
}

#[derive(Serialize)]
struct VmessConnection {
    v: String,
    ps: String,
    add: String,
    port: String,
    id: String,
    aid: String,
    scy: String,
    net: String,
    r#type: String,
    host: String,
    path: String,
    tls: String,
}

pub fn vmess_tcp_conn(
    conn_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
) -> PonyResult<String> {
    let port = inbound.port;
    let stream_settings = inbound
        .stream_settings
        .ok_or(PonyError::Custom("VMESS: stream settings error".into()))?;
    let tcp_settings = stream_settings
        .tcp_settings
        .ok_or(PonyError::Custom("VMESS: stream tcp settings error".into()))?;
    let header = tcp_settings
        .header
        .ok_or(PonyError::Custom("VMESS: header tcp settings error".into()))?;
    let req = header
        .request
        .ok_or(PonyError::Custom("VMESS: header req settings error".into()))?;
    let headers = req
        .headers
        .ok_or(PonyError::Custom("VMESS: headers settings error".into()))?;

    let host = headers
        .get("Host")
        .ok_or(PonyError::Custom("VMESS: host settings error".into()))?
        .first()
        .ok_or(PonyError::Custom(
            "VMESS: Host stream settings error".into(),
        ))?;
    let path = req
        .path
        .first()
        .ok_or(PonyError::Custom("VMESS: path settings error".into()))?;

    let conn = VmessConnection {
        v: "2".into(),
        ps: format!("Vmess {}", label),
        add: ipv4.to_string(),
        port: port.to_string(),
        id: conn_id.to_string(),
        aid: "0".into(),
        scy: "auto".into(),
        net: "tcp".into(),
        r#type: "http".into(),
        host: host.to_string(),
        path: path.to_string(),
        tls: "none".into(),
    };

    let json_str = serde_json::to_string(&conn)
        .ok()
        .ok_or(PonyError::Custom("VMESS serde json error".into()))?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(json_str);

    Ok(format!("vmess://{base64_str}#{label} ____"))
}
