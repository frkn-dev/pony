use super::ProtocolConn;
use crate::memory::tag::ProtoTag as Tag;
use crate::xray_api::xray::proxy::shadowsocks;
use crate::xray_api::xray::{
    common::protocol::User, common::serial::TypedMessage, proxy::shadowsocks::CipherType,
};

#[derive(Clone, Debug)]
pub struct ConnInfo {
    pub uuid: uuid::Uuid,
    pub cipher_type: String,
    pub in_tag: Tag,
    pub level: u32,
    pub email: String,
    pub password: Option<String>,
}

impl ConnInfo {
    pub fn new(uuid: &uuid::Uuid, password: Option<String>) -> Self {
        Self {
            uuid: *uuid,
            in_tag: Tag::Shadowsocks,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            password: password,
            cipher_type: "chacha20-ietf-poly1305".to_string(),
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
        let cipher_type = match self.cipher_type.as_str() {
            "aes-128-gcm" => CipherType::Aes128Gcm,
            "aes-256-gcm" => CipherType::Aes256Gcm,
            "chacha20-ietf-poly1305" => CipherType::Chacha20Poly1305,
            _ => return Err("Unsupported cipher type".into()),
        };

        let account = shadowsocks::Account {
            password: self.password.clone().ok_or("Missing password")?,
            cipher_type: cipher_type as i32,
            iv_check: false,
        };

        Ok(User {
            level: self.level,
            email: self.email.clone(),
            account: Some(TypedMessage {
                r#type: "xray.proxy.shadowsocks.Account".to_string(),
                value: prost::Message::encode_to_vec(&account),
            }),
        })
    }
}
