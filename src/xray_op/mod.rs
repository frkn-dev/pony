pub mod client;
pub mod connections;
pub mod shadowsocks;
pub mod stats;
pub mod vless;
pub mod vmess;

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::Request;

use crate::state::Tag;
use crate::xray_api::xray::{
    app::proxyman::command::{AddUserOperation, AlterInboundRequest, RemoveUserOperation},
    common::protocol::User,
    common::serial::TypedMessage,
};
use crate::xray_op::client::HandlerClient;

#[async_trait::async_trait]
pub trait ProtocolConn: Send + Sync {
    fn tag(&self) -> Tag;
    fn email(&self) -> String;
    fn to_user(&self) -> Result<User, Box<dyn std::error::Error + Send + Sync>>;

    async fn create(&self, client: Arc<Mutex<HandlerClient>>) -> Result<(), tonic::Status> {
        let request = AlterInboundRequest {
            tag: self.tag().to_string(),
            operation: Some(TypedMessage {
                r#type: "xray.app.proxyman.command.AddUserOperation".to_string(),
                value: prost::Message::encode_to_vec(&AddUserOperation {
                    user: Some(self.to_user().expect("REASON")),
                }),
            }),
        };

        let mut locked = client.lock().await;
        locked
            .client
            .alter_inbound(Request::new(request))
            .await
            .map(|_| ())
    }

    async fn remove(&self, client: Arc<Mutex<HandlerClient>>) -> Result<(), tonic::Status> {
        let operation = RemoveUserOperation {
            email: self.email(),
        };

        let operation_message = TypedMessage {
            r#type: "xray.app.proxyman.command.RemoveUserOperation".to_string(),
            value: prost::Message::encode_to_vec(&operation),
        };

        let request = AlterInboundRequest {
            tag: self.tag().to_string(),
            operation: Some(operation_message),
        };

        let mut handler_client = client.lock().await;
        handler_client
            .client
            .alter_inbound(Request::new(request))
            .await
            .map(|_| ())
    }
}
