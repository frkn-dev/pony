use std::fmt;
use tonic::Request;
use uuid::Uuid;

use super::{client::XrayClients, Tag};

use crate::xray_api::xray::{
    app::proxyman::command::{AddUserOperation, AlterInboundRequest},
    common::protocol::User,
    common::serial::TypedMessage,
    proxy::vless::Account,
};

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

pub async fn add_user(clients: XrayClients, user_info: UserInfo) -> Result<(), tonic::Status> {
    let vless_account = Account {
        id: user_info.uuid.to_string(),
        flow: user_info.flow.to_string(),
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
        tag: user_info.in_tag.to_string(),
        operation: Some(operation_message),
    };

    let mut handler_client = clients.handler_client.lock().await;

    handler_client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
