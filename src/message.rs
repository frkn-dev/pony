use log::{debug, error, info};
use serde::Deserialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::zmq::Action;
use crate::{
    utils::generate_random_password,
    xray_op::{client, remove_user, shadowsocks, user_state, users, vless, vmess, Tag},
};

#[derive(Deserialize, Clone, Debug)]
pub struct Message {
    pub user_id: String,
    pub action: Action,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    state: Arc<Mutex<user_state::UserState>>,
    config_daily_limit_mb: i64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match message.action {
        Action::Create => {
            let mut user_state = state.lock().await;

            let daily_limit_mb = message.limit.unwrap_or(config_daily_limit_mb);
            let trial = message.trial.unwrap_or(true);

            let password = match message.password {
                Some(password) => password,
                None => generate_random_password(10),
            };

            let user = users::User::new(
                message.user_id.clone(),
                daily_limit_mb,
                trial,
                password.clone(),
            );

            if let Err(e) = user_state.add_user(user.clone()).await {
                error!("Create: Fail to add user to State: {:?}", e);
                return Err(
                    format!("Create: Failed to add user {} to state", message.user_id).into(),
                );
            }

            let user_info = vmess::UserInfo::new(message.user_id.to_string());
            if let Err(e) = vmess::add_user(clients.clone(), user_info.clone()).await {
                error!("Create: Fail to add Vmess user: {:?}", e);
            } else {
                debug!("Create: Success to add Vmess user: {:?}", message.user_id);
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::Vmess);
                }
            }

            let user_info =
                vless::UserInfo::new(message.user_id.to_string(), vless::UserFlow::Vision);
            if let Err(e) = vless::add_user(clients.clone(), user_info.clone()).await {
                error!("Create: Fail to add VlessXtls  user: {:?}", e);
            } else {
                debug!(
                    "Create: Success to add VlessXtls user: {:?}",
                    message.user_id
                );
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::VlessXtls);
                }
            }

            let user_info =
                vless::UserInfo::new(message.user_id.to_string(), vless::UserFlow::Direct);
            if let Err(e) = vless::add_user(clients.clone(), user_info.clone()).await {
                error!("Create: Fail to add VlessGrpc user: {:?}", e);
            } else {
                debug!(
                    "Create: Success to add VlessGrpc user: {:?}",
                    message.user_id
                );
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::VlessGrpc);
                }
            }

            let user_info = shadowsocks::UserInfo::new(message.user_id.to_string(), password);
            if let Err(e) = shadowsocks::add_user(clients.clone(), user_info.clone()).await {
                error!("Create: Fail to add Shadowsocks user: {:?}", e);
            } else {
                debug!(
                    "Create: Success to add Shadowsocks user: {:?}",
                    message.user_id
                );
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::Shadowsocks);
                }
            }

            info!("Create: User added: {:?}", message.user_id);

            let _ = user_state.save_to_file_async().await;

            Ok(())
        }
        Action::Delete => {
            let mut user_state = state.lock().await;

            if let Err(e) = remove_user(clients.clone(), message.user_id.clone(), Tag::Vmess).await
            {
                error!("Delete: Failed to remove Vmess user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.remove_proto(Tag::Vmess);
                }
            }

            if let Err(e) =
                remove_user(clients.clone(), message.user_id.clone(), Tag::VlessXtls).await
            {
                error!("Delete: Failed to remove VlessXtls user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.remove_proto(Tag::VlessXtls);
                }
            }

            if let Err(e) =
                remove_user(clients.clone(), message.user_id.clone(), Tag::VlessGrpc).await
            {
                error!("Delete: Failed to remove VlessGrpc user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.remove_proto(Tag::VlessXtls);
                }
            }

            if let Err(e) =
                remove_user(clients.clone(), message.user_id.clone(), Tag::Shadowsocks).await
            {
                error!("Delete: Failed to remove Shadowsocks user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.remove_proto(Tag::Shadowsocks);
                }
            }

            let _ = user_state.expire_user(&message.user_id).await;

            Ok(())
        }
        Action::Restore => {
            let mut user_state = state.lock().await;

            let user_info = vmess::UserInfo::new(message.user_id.to_string());
            if let Err(e) = vmess::add_user(clients.clone(), user_info.clone()).await {
                error!("Restore: Failed to restore Vmess user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::Vmess);
                }
            }

            let user_info =
                vless::UserInfo::new(message.user_id.to_string(), vless::UserFlow::Vision);
            if let Err(e) = vless::add_user(clients.clone(), user_info.clone()).await {
                error!("Failed to restore VlessXtls user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::VlessXtls);
                }
            }

            let user_info =
                vless::UserInfo::new(message.user_id.to_string(), vless::UserFlow::Direct);
            if let Err(e) = vless::add_user(clients.clone(), user_info.clone()).await {
                error!("Failed to restore VlessGrpc user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::VlessXtls);
                }
            }

            if let Some(password) = user_state.get_user_password(&message.user_id) {
                let user_info = shadowsocks::UserInfo::new(message.user_id.to_string(), password);
                if let Err(e) = shadowsocks::add_user(clients.clone(), user_info).await {
                    error!("Failed to restore Shadowsocks user: {:?}", e);
                } else {
                    if let Some(existing_user) = user_state
                        .users
                        .iter_mut()
                        .find(|u| u.user_id == message.user_id)
                    {
                        existing_user.add_proto(Tag::Shadowsocks);
                    }
                }
            }

            let _ = user_state.restore_user(message.user_id.clone()).await;
            info!("Restore: user restored: {:?}", message.user_id);

            Ok(())
        }
        Action::Update => {
            if let Some(trial) = message.trial {
                debug!("Update user trial {} {}", message.user_id, trial);
                let mut user_state = state.lock().await;
                let _ = user_state.update_user_trial(&message.user_id, trial).await;
            }
            if let Some(limit) = message.limit {
                debug!("Update user limit {} {}", message.user_id, limit);
                let mut user_state = state.lock().await;
                let _ = user_state.update_user_limit(&message.user_id, limit).await;
            }
            Ok(())
        }
    }
}
