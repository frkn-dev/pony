use log::{debug, error};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Status};
use uuid::Uuid;

use super::{client::HandlerClient, shadowsocks, vless, vmess};
use crate::state::tag::Tag;
use crate::xray_api::xray::{
    app::proxyman::command::{AlterInboundRequest, RemoveUserOperation},
    common::serial::TypedMessage,
};

pub async fn create_users(
    user_id: Uuid,
    password: Option<String>,
    client: Arc<Mutex<HandlerClient>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("CREATE USER");

    let _ = {
        let client = client.clone();
        let user_info = vmess::UserInfo::new(user_id);
        match vmess::add_user(client.clone(), user_info.clone()).await {
            Ok(_) => debug!(
                "Create: Success to add {:?} user: {:?}",
                user_info.in_tag, user_info.uuid
            ),
            Err(e) => error!(
                "Create: Error to add {}  user: {:?}: {:?}",
                user_info.in_tag, user_info.uuid, e
            ),
        }
    };

    let _ = {
        let client = client.clone();
        let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Vision);
        match vless::add_user(client.clone(), user_info.clone()).await {
            Ok(_) => debug!(
                "Create: Success to add {:?} user: {:?}",
                user_info.in_tag, user_info.uuid
            ),
            Err(e) => error!(
                "Create: Error to add {}  user: {:?}: {:?}",
                user_info.in_tag, user_info.uuid, e
            ),
        }
    };

    let _ = {
        let client = client.clone();
        let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Direct);
        match vless::add_user(client.clone(), user_info.clone()).await {
            Ok(_) => debug!(
                "Create: Success to add {:?} user: {:?}",
                user_info.in_tag, user_info.uuid
            ),
            Err(e) => error!(
                "Create: Error to add {}  user: {:?}: {:?}",
                user_info.in_tag, user_info.uuid, e
            ),
        }
    };

    let _ = {
        let client = client.clone();
        if let Some(password) = password {
            let user_info = shadowsocks::UserInfo::new(user_id, Some(password));
            match shadowsocks::add_user(client.clone(), user_info.clone()).await {
                Ok(_) => debug!(
                    "Create: Success to add {:?} user: {:?}",
                    user_info.in_tag, user_info.uuid
                ),
                Err(e) => error!(
                    "Create: Error to add {}  user: {:?}: {:?}",
                    user_info.in_tag, user_info.uuid, e
                ),
            }
        }
    };

    println!("user created {}", user_id);

    Ok(())
}

pub async fn remove_users(
    user_id: Uuid,
    client: Arc<Mutex<HandlerClient>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(e) = remove_user(client.clone(), user_id, Tag::Vmess).await {
        error!("Delete: Failed to remove Vmess user: {:?}", e);
    }

    if let Err(e) = remove_user(client.clone(), user_id.clone(), Tag::VlessXtls).await {
        error!("Delete: Failed to remove VlessXtls user: {:?}", e);
    }

    if let Err(e) = remove_user(client.clone(), user_id.clone(), Tag::VlessGrpc).await {
        error!("Delete: Failed to remove VlessGrpc user: {:?}", e);
    }

    if let Err(e) = remove_user(client, user_id, Tag::Shadowsocks).await {
        error!("Delete: Failed to remove Shadowsocks user: {:?}", e);
    }

    Ok(())
}

pub async fn remove_user<Tag>(
    client: Arc<Mutex<HandlerClient>>,
    user_id: Uuid,
    tag: Tag,
) -> Result<(), Status>
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

    let mut handler_client = client.lock().await;

    handler_client
        .client
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
