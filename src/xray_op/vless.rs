use super::client::XrayClients;
use super::Tag;
use tonic::Request;

use crate::xray_api::xray::app::proxyman::command::{
    AddUserOperation, AlterInboundRequest, RemoveUserOperation,
};
use crate::xray_api::xray::common::protocol::User;
use crate::xray_api::xray::common::serial::TypedMessage;
use crate::xray_api::xray::proxy::vless::Account;

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub in_tag: String,
    pub level: u32,
    pub email: String,
    pub uuid: String,
    encryption: Option<String>,
    flow: Option<String>,
}

impl UserInfo {
    pub fn new(uuid: String, in_tag: Tag) -> Self {
        Self {
            in_tag: in_tag.to_string(),
            level: 0,
            email: format!("{}@{}", uuid, in_tag),
            uuid: uuid,
            encryption: Some("none".to_string()),
            flow: Some("xtls-rprx-direct".to_string()),
        }
    }
}

pub async fn add_user(clients: XrayClients, user_info: UserInfo) -> Result<(), tonic::Status> {
    let vless_account = Account {
        id: user_info.uuid.clone(),
        flow: user_info.flow.unwrap_or_default(),
        encryption: user_info.encryption.unwrap_or_else(|| "none".to_string()),
    };

    let vless_account_bytes = prost::Message::encode_to_vec(&vless_account);

    let user = User {
        level: user_info.level,
        email: user_info.email.clone(),
        account: Some(TypedMessage {
            r#type: "xray.proxy.vless.Account".to_string(),
            value: vless_account_bytes,
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

pub async fn remove_user(clients: XrayClients, user_id: String) -> Result<(), tonic::Status> {
    let tag: Tag = Tag::Vless;

    let operation = RemoveUserOperation {
        email: format!("{}@{}", user_id, tag),
    };

    let operation_message = TypedMessage {
        r#type: "xray.app.proxyman.command.RemoveUserOperation".to_string(),
        value: prost::Message::encode_to_vec(&operation),
    };

    let request = AlterInboundRequest {
        tag: tag.to_string(),
        operation: Some(operation_message),
    };

    let mut handler_client = clients.handler_client.lock().await;
    handler_client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
