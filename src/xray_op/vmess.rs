use crate::xray_api::xray::app::proxyman::command::{
    handler_service_client::HandlerServiceClient, AddUserOperation, AlterInboundRequest,
    RemoveUserOperation,
};

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Request;

use crate::xray_api::xray::common::protocol::User;
use crate::xray_api::xray::common::serial::TypedMessage;
use crate::xray_api::xray::proxy::vmess::Account;

#[derive(Clone)]
pub struct UserInfo {
    pub in_tag: String,
    pub level: u32,
    pub email: String,
    pub uuid: String,
}

pub async fn add_user(
    client: Arc<Mutex<HandlerServiceClient<Channel>>>,
    user_info: UserInfo,
) -> Result<(), tonic::Status> {
    let vmess_account = Account {
        id: user_info.uuid.clone(),
        security_settings: None,
        tests_enabled: String::new(),
    };

    let vmess_account_bytes = prost::Message::encode_to_vec(&vmess_account);

    let user = User {
        level: user_info.level,
        email: user_info.email.clone(),
        account: Some(TypedMessage {
            r#type: "xray.proxy.vmess.Account".to_string(),
            value: vmess_account_bytes,
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

    let mut client = client.lock().await;

    client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}

pub async fn remove_user(
    client: Arc<Mutex<HandlerServiceClient<Channel>>>,
    user_info: UserInfo,
) -> Result<(), tonic::Status> {
    let operation = RemoveUserOperation {
        email: user_info.email,
    };

    let operation_message = TypedMessage {
        r#type: "xray.app.proxyman.command.RemoveUserOperation".to_string(),
        value: prost::Message::encode_to_vec(&operation),
    };

    let request = AlterInboundRequest {
        tag: user_info.in_tag,
        operation: Some(operation_message),
    };

    let mut client = client.lock().await;
    client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
