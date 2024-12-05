use crate::xray_api::xray::app::proxyman::command::{
    AddUserOperation, AlterInboundRequest, RemoveUserOperation,
};

use tonic::Request;

use crate::xray_api::xray::common::protocol::User;
use crate::xray_api::xray::common::serial::TypedMessage;
use crate::xray_api::xray::proxy::vmess::Account;
use crate::xray_op::client::XrayClients;
use crate::xray_op::users::UserInfo;
use crate::zmq::Tag;

pub async fn add_user(clients: XrayClients, user_info: UserInfo) -> Result<(), tonic::Status> {
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

    let mut handler_client = clients.handler_client.lock().await;

    handler_client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}

pub async fn remove_user(
    clients: XrayClients,
    user_email: String,
    tag: Tag,
) -> Result<(), tonic::Status> {
    let operation = RemoveUserOperation { email: user_email };

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
