use tonic::{Request, Status};

use crate::utils::generate_random_password;
use crate::xray_api::xray::{
    app::proxyman::command::{AddUserOperation, AlterInboundRequest},
    common::protocol::User,
    common::serial::TypedMessage,
    proxy::shadowsocks::Account,
    proxy::shadowsocks::CipherType,
};

use super::{client::XrayClients, Tag};

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub cipher_type: String,
    pub in_tag: Tag,
    pub level: u32,
    pub email: String,
    pub password: Option<String>,
}

impl UserInfo {
    pub fn new(uuid: String, password: String) -> Self {
        Self {
            in_tag: Tag::Shadowsocks,
            level: 0,
            email: format!("{}@{}", uuid, "pony"),
            password: Some(password),
            cipher_type: "chacha20-ietf-poly1305".to_string(),
        }
    }
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            email: String::default(),
            in_tag: Tag::Shadowsocks,
            password: Some(generate_random_password(10)),
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

    let password = match user_info.password {
        Some(password) => password,
        None => return Err(Status::new(403.into(), "Password is ommited")),
    };

    let ss_account = Account {
        password: password,
        cipher_type: ss_cipher_type as i32,
        iv_check: false,
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
        tag: user_info.in_tag.to_string(),
        operation: Some(operation_message),
    };

    let mut handler_client = clients.handler_client.lock().await;

    handler_client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
