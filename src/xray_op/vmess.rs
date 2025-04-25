use uuid::Uuid;

use crate::xray_api::xray::proxy::vmess;
use crate::xray_api::xray::{common::protocol::User, common::serial::TypedMessage};
use crate::ProtocolUser;
use crate::Tag;

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub in_tag: Tag,
    pub level: u32,
    pub email: String,
    pub uuid: Uuid,
}

impl UserInfo {
    pub fn new(uuid: Uuid) -> Self {
        Self {
            in_tag: Tag::Vmess,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            uuid: uuid,
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
