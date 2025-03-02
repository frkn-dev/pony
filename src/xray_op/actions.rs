use log::{debug, error};
use std::error::Error;
use tonic::{Request, Status};
use uuid::Uuid;

use super::user;
use super::{client::HandlerClient, client::XrayClient, shadowsocks, vless, vmess};
use crate::state::tag::Tag;
use crate::xray_api::xray::{
    app::proxyman::command::{AlterInboundRequest, RemoveUserOperation},
    common::serial::TypedMessage,
};

async fn user_exist(client: HandlerClient, uuid: Uuid, in_tag: Tag) -> bool {
    match user::get_user(client, in_tag, uuid.to_string()).await {
        Ok(user_exist) => user_exist
            .users
            .iter()
            .find(|user| user.email.is_empty())
            .is_some(),
        Err(_) => false,
    }
}

pub async fn create_users(
    user_id: Uuid,
    password: Option<String>,
    client: HandlerClient,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("CREATE USER");

    let user_info = vmess::UserInfo::new(user_id);
    if user_exist(client.clone(), user_info.uuid, user_info.in_tag.clone()).await {
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
    } else {
        debug!(
            "User already exist: {} {:?}",
            user_id,
            user_info.in_tag.clone()
        );
    }

    let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Vision);
    if user_exist(client.clone(), user_info.uuid, user_info.in_tag.clone()).await {
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
    } else {
        debug!(
            "User already exist: {} {:?}",
            user_id,
            user_info.in_tag.clone()
        );
    }

    let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Direct);
    if user_exist(client.clone(), user_info.uuid, user_info.in_tag.clone()).await {
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
    } else {
        debug!(
            "User already exist: {} {:?}",
            user_id,
            user_info.in_tag.clone()
        );
    }

    if let Some(password) = password {
        let user_info = shadowsocks::UserInfo::new(user_id, Some(password));
        if user_exist(client.clone(), user_info.uuid, user_info.in_tag.clone()).await {
            match shadowsocks::add_user(client, user_info.clone()).await {
                Ok(_) => debug!(
                    "Create: Success to add {:?} user: {:?}",
                    user_info.in_tag, user_info.uuid
                ),
                Err(e) => error!(
                    "Create: Error to add {}  user: {:?}: {:?}",
                    user_info.in_tag, user_info.uuid, e
                ),
            }
        } else {
            debug!(
                "User already exist: {} {:?}",
                user_id,
                user_info.in_tag.clone()
            );
        }
    }

    println!("user created {}", user_id);

    Ok(())
}

pub async fn remove_users(
    user_id: Uuid,
    client: HandlerClient,
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

pub async fn remove_user<Tag>(client: HandlerClient, user_id: Uuid, tag: Tag) -> Result<(), Status>
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
        .alter_inbound(Request::new(request))
        .await
        .map(|_| ())
}
