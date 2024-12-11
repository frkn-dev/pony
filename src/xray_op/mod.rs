use crate::xray_api::xray::{
    app::proxyman::command::{AlterInboundRequest, RemoveUserOperation},
    common::serial::TypedMessage,
};
use crate::xray_op::client::XrayClients;
use serde::{Deserialize, Serialize};
use std::fmt;
use tonic::{Request, Status};

pub mod client;
pub mod config;
pub mod node;
pub mod shadowsocks;
pub mod stats;
pub mod user;
pub mod vless;
pub mod vmess;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Tag {
    #[serde(rename = "vlessXtls")]
    VlessXtls,
    #[serde(rename = "vlessGrpc")]
    VlessGrpc,
    #[serde(rename = "vmess")]
    Vmess,
    #[serde(rename = "shadowsocks")]
    Shadowsocks,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::VlessXtls => write!(f, "VlessXtls"),
            Tag::VlessGrpc => write!(f, "VlessGrpc"),
            Tag::Vmess => write!(f, "Vmess"),
            Tag::Shadowsocks => write!(f, "Shadowsocks"),
        }
    }
}

impl std::str::FromStr for Tag {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "VlessXtls" => Ok(Tag::VlessXtls),
            "VlessGrpc" => Ok(Tag::VlessGrpc),
            "Vmess" => Ok(Tag::Vmess),
            "Shadowsocks" => Ok(Tag::Shadowsocks),
            _ => Err(()),
        }
    }
}

pub async fn remove_user<Tag>(clients: XrayClients, user_id: String, tag: Tag) -> Result<(), Status>
where
    Tag: ToString,
{
    let operation = RemoveUserOperation {
        email: format!("{}@{}", user_id, "pony"),
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
