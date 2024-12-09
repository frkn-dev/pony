use tonic::{Request, Status};

use crate::utils::generate_random_password;
use crate::xray_api::xray::app::proxyman::command::{AddUserOperation, AlterInboundRequest};
use crate::xray_api::xray::common::protocol::User;
use crate::xray_api::xray::common::serial::TypedMessage;
use crate::xray_api::xray::proxy::shadowsocks::Account;
use crate::xray_api::xray::proxy::shadowsocks::CipherType;

use super::client::XrayClients;
use super::Tag;

#[derive(Clone)]
pub struct UserInfo {
    pub cipher_type: String,
    pub in_tag: String,
    pub level: u32,
    pub email: String,
    pub password: String,
}

impl UserInfo {
    pub fn new(uuid: String, in_tag: Tag, password: String) -> Self {
        Self {
            in_tag: in_tag.to_string(),
            level: 0,
            email: format!("{}@{}", uuid, in_tag),
            password: password,
            cipher_type: "chacha20-ietf-poly1305".to_string(),
        }
    }
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            email: String::default(),
            in_tag: Tag::Shadowsocks.to_string(),
            password: generate_random_password(10),
            level: 0,
            cipher_type: "chacha20-ietf-poly1305".to_string(),
        }
    }
}

pub async fn add_user(clients: XrayClients, user_info: UserInfo) -> Result<(), tonic::Status> {
    let ss_cipher_type = match user_info.cipher_type.as_str() {
        "aes-128-gcm" => CipherType::Aes128Gcm,
        "aes-256-gcm" => CipherType::Aes256Gcm,
        "chacha20-ietf-poly1305" => CipherType::Chacha20Poly1305,
        _ => {
            return Err(Status::new(404.into(), "Unsupported cipher type"));
        }
    };

    let ss_account = Account {
        password: user_info.password.clone(),
        cipher_type: ss_cipher_type as i32,
        iv_check: true,
    };

    let ss_account_bytes = prost::Message::encode_to_vec(&ss_account);

    let user = User {
        level: user_info.level,
        email: user_info.email.clone(),
        account: Some(TypedMessage {
            r#type: "xray.proxy.shadowsocks.Account".to_string(),
            value: ss_account_bytes,
        }),
    };

    let add_user_operation = AddUserOperation { user: Some(user) };

    let add_user_operation_bytes = prost::Message::encode_to_vec(&add_user_operation);

    let operation_message = TypedMessage {
        r#type: "xray.app.proxyman.command.AddUserOperation".to_string(),
        value: add_user_operation_bytes,
    };

    let request = AlterInboundRequest {
        tag: user_info.in_tag.clone(),
        operation: Some(operation_message),
    };

    let mut handler_client = clients.handler_client.lock().await;

    handler_client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
