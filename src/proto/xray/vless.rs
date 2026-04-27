use std::fmt;

use super::api::proxy::vless;
use super::api::{common::protocol::User, common::serial::TypedMessage};
use crate::memory::tag::ProtoTag as Tag;

use super::client::ProtocolConn;

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
    pub fn new(uuid: &uuid::Uuid, flow: ConnFlow, tag: Tag) -> Self {
        Self {
            in_tag: tag,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            uuid: *uuid,
            encryption: Some("none".to_string()),
            flow,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConnFlow {
    Vision,
    Direct,
    None,
}

impl fmt::Display for ConnFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnFlow::Vision => write!(f, "xtls-rprx-vision"),
            ConnFlow::Direct => write!(f, "xtls-rprx-direct"),
            ConnFlow::None => write!(f, ""),
        }
    }
}

#[async_trait::async_trait]
impl ProtocolConn for ConnInfo {
    fn tag(&self) -> Tag {
        self.in_tag
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
