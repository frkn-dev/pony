use std::fmt;

use crate::xray_api::xray::proxy::vless;
use crate::xray_api::xray::{common::protocol::User, common::serial::TypedMessage};
use crate::xray_op::ProtocolConn;
use crate::xray_op::Tag;

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
