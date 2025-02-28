use log::{debug, error};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Status};
use uuid::Uuid;

use super::user;
use super::{client::XrayClients, shadowsocks, vless, vmess};
use crate::state::{state::State, tag::Tag};

use crate::xray_api::xray::{
    app::proxyman::command::{AlterInboundRequest, RemoveUserOperation},
    common::serial::TypedMessage,
};

async fn user_exist(clients: XrayClients, uuid: Uuid, in_tag: Tag) -> bool {
    match user::get_user(clients.clone(), in_tag, uuid.to_string()).await {
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
    clients: XrayClients,
    state: Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("CREATE USER");

    let user_info = vmess::UserInfo::new(user_id);
    if user_exist(clients.clone(), user_info.uuid, user_info.in_tag.clone()).await {
        if let Err(e) = vmess::add_user(clients.clone(), user_info.clone()).await {
            error!("Create: Fail to add {}  user: {:?}", user_info.in_tag, e);
        } else {
            debug!(
                "Create: Success to add {} user: {:?}",
                user_info.in_tag, user_id
            );
            let mut state_lock = state.lock().await;
            if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
                existing_user.add_proto(user_info.in_tag);
            }
        }
    } else {
        debug!(
            "User already exist: {} {:?}",
            user_id,
            user_info.in_tag.clone()
        );

        let mut state_lock = state.lock().await;
        if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
            existing_user.add_proto(user_info.in_tag);
        }
    }

    let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Vision);
    if user_exist(clients.clone(), user_info.uuid, user_info.in_tag.clone()).await {
        if let Err(e) = vless::add_user(clients.clone(), user_info.clone()).await {
            error!("Create: Fail to add {}  user: {:?}", user_info.in_tag, e);
        } else {
            debug!(
                "Create: Success to add {} user: {:?}",
                user_info.in_tag, user_id
            );
            let mut state_lock = state.lock().await;
            if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
                existing_user.add_proto(user_info.in_tag);
            }
        }
    } else {
        debug!(
            "User already exist: {} {:?}",
            user_id,
            user_info.in_tag.clone()
        );

        let mut state_lock = state.lock().await;
        if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
            existing_user.add_proto(user_info.in_tag);
        }
    }

    let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Direct);
    if user_exist(clients.clone(), user_info.uuid, user_info.in_tag.clone()).await {
        if let Err(e) = vless::add_user(clients.clone(), user_info.clone()).await {
            error!("Create: Fail to add {}  user: {:?}", user_info.in_tag, e);
        } else {
            debug!(
                "Create: Success to add {} user: {:?}",
                user_info.in_tag, user_id
            );
            let mut state_lock = state.lock().await;
            if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
                existing_user.add_proto(Tag::VlessGrpc);
            }
        }
    } else {
        debug!(
            "User already exist: {} {:?}",
            user_id,
            user_info.in_tag.clone()
        );

        let mut state_lock = state.lock().await;
        if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
            existing_user.add_proto(user_info.in_tag);
        }
    }

    if let Some(password) = password {
        let user_info = shadowsocks::UserInfo::new(user_id, Some(password));
        if user_exist(clients.clone(), user_info.uuid, user_info.in_tag.clone()).await {
            if let Err(e) = shadowsocks::add_user(clients.clone(), user_info.clone()).await {
                error!("Create: Fail to add {}  user: {:?}", user_info.in_tag, e);
            } else {
                debug!(
                    "Create: Success to add {} user: {:?}",
                    user_info.in_tag, user_id
                );

                let mut state_lock = state.lock().await;
                if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
                    existing_user.add_proto(Tag::Shadowsocks);
                }
            }
        } else {
            debug!(
                "User already exist: {} {:?}",
                user_id,
                user_info.in_tag.clone()
            );
            let mut state_lock = state.lock().await;
            if let Some(existing_user) = state_lock.users.get_mut(&user_id) {
                existing_user.add_proto(user_info.in_tag);
            }
        }
    }

    Ok(())
}

pub async fn remove_users(
    user_id: Uuid,
    state: Arc<Mutex<State>>,
    clients: XrayClients,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut state = state.lock().await;

    if let Err(e) = remove_user(clients.clone(), user_id, Tag::Vmess).await {
        error!("Delete: Failed to remove Vmess user: {:?}", e);
    } else {
        if let Some(existing_user) = state.users.get_mut(&user_id) {
            existing_user.remove_proto(Tag::Vmess);
        }
    }

    if let Err(e) = remove_user(clients.clone(), user_id.clone(), Tag::VlessXtls).await {
        error!("Delete: Failed to remove VlessXtls user: {:?}", e);
    } else {
        if let Some(existing_user) = state.users.get_mut(&user_id) {
            existing_user.remove_proto(Tag::VlessXtls);
        }
    }

    if let Err(e) = remove_user(clients.clone(), user_id.clone(), Tag::VlessGrpc).await {
        error!("Delete: Failed to remove VlessGrpc user: {:?}", e);
    } else {
        if let Some(existing_user) = state.users.get_mut(&user_id) {
            existing_user.remove_proto(Tag::VlessGrpc);
        }
    }

    if let Err(e) = remove_user(clients.clone(), user_id, Tag::Shadowsocks).await {
        error!("Delete: Failed to remove Shadowsocks user: {:?}", e);
    } else {
        if let Some(existing_user) = state.users.get_mut(&user_id) {
            existing_user.remove_proto(Tag::Shadowsocks);
        }
    }

    Ok(())
}

pub async fn remove_user<Tag>(clients: XrayClients, user_id: Uuid, tag: Tag) -> Result<(), Status>
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
