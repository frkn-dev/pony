use std::fmt;
use uuid::Uuid;

use crate::xray_api::xray::proxy::vless;
use crate::xray_api::xray::{common::protocol::User, common::serial::TypedMessage};
use crate::ProtocolUser;
use crate::Tag;

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub uuid: Uuid,
    pub in_tag: Tag,
    pub level: u32,
    pub email: String,
    encryption: Option<String>,
    flow: UserFlow,
}

impl UserInfo {
    pub fn new(uuid: Uuid, flow: UserFlow) -> Self {
        let tag = match flow {
            UserFlow::Vision => Tag::VlessXtls,
            UserFlow::Direct => Tag::VlessGrpc,
        };

        Self {
            in_tag: tag,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            uuid: uuid,
            encryption: Some("none".to_string()),
            flow: flow,
        }
    }
}

#[derive(Clone, Debug)]
pub enum UserFlow {
    Vision,
    Direct,
}

impl fmt::Display for UserFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserFlow::Vision => write!(f, "xtls-rprx-vision"),
            UserFlow::Direct => write!(f, "xtls-rprx-direct"),
        }
    }
}

#[async_trait::async_trait]
impl ProtocolUser for UserInfo {
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
