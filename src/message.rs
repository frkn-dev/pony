use log::{debug, error, info};
use serde::Deserialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::xray_op::{client, users, vless, vmess, Tag};
use super::zmq::Action;
use crate::xray_op::user_state;

#[derive(Deserialize, Clone, Debug)]
pub struct Message {
    pub user_id: String,
    pub action: Action,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    state: Arc<Mutex<user_state::UserState>>,
    config_daily_limit_mb: i64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let vmess_user_info = vmess::UserInfo::new(message.user_id.to_string(), Tag::Vmess);
    let vless_user_info = vless::UserInfo::new(message.user_id.to_string(), Tag::Vless);

    match message.action {
        Action::Create => {
            let mut user_state = state.lock().await;

            let daily_limit_mb = message.limit.unwrap_or(config_daily_limit_mb);
            let trial = message.trial.unwrap_or(true);
            let user = users::User::new(message.user_id.clone(), daily_limit_mb, trial);

            if let Err(e) = user_state.add_user(user.clone()).await {
                error!("Create: Fail to add user to State: {:?}", e);
            }

            if let Err(e) = vmess::add_user(clients.clone(), vmess_user_info.clone()).await {
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

            if let Err(e) = vless::add_user(clients.clone(), vless_user_info.clone()).await {
                error!("Create: Fail to add Vless user: {:?}", e);
            } else {
                debug!("Create: Success to add Vless user: {:?}", message.user_id);
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::Vless);
                }
            }

            info!("Create: User added: {:?}", message.user_id);

            Ok(())
        }
        Action::Delete => {
            let mut user_state = state.lock().await;

            if let Err(e) = vmess::remove_user(clients.clone(), message.user_id.clone()).await {
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

            if let Err(e) = vless::remove_user(clients.clone(), message.user_id.clone()).await {
                error!("Delete: Failed to remove Vless user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.remove_proto(Tag::Vless);
                }
            }

            let _ = user_state.expire_user(&message.user_id).await;

            Ok(())
        }
        Action::Restore => {
            let mut user_state = state.lock().await;

            let daily_limit_mb = message.limit.unwrap_or(config_daily_limit_mb);
            let trial = message.trial.unwrap_or(true);

            if let Err(e) = vmess::add_user(clients.clone(), vmess_user_info.clone()).await {
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

            if let Err(e) = vless::add_user(clients.clone(), vless_user_info.clone()).await {
                error!("Failed to restore Vless user: {:?}", e);
            } else {
                if let Some(existing_user) = user_state
                    .users
                    .iter_mut()
                    .find(|u| u.user_id == message.user_id)
                {
                    existing_user.add_proto(Tag::Vless);
                }
            }

            let user = users::User::new(message.user_id.clone(), daily_limit_mb, trial);
            let _ = user_state.restore_user(user).await;
            info!("Restore: user restored: {:?}", message.user_id);

            Ok(())
        }
        Action::Update => {
            if let Some(trial) = message.trial {
                debug!("Update user trial {} {}", vmess_user_info.uuid, trial);
                let mut user_state = state.lock().await;
                let _ = user_state
                    .update_user_trial(&vmess_user_info.uuid, trial)
                    .await;
            }
            if let Some(limit) = message.limit {
                debug!("Update user limit {} {}", vmess_user_info.uuid, limit);
                let mut user_state = state.lock().await;
                let _ = user_state
                    .update_user_limit(&vmess_user_info.uuid, limit)
                    .await;
            }
            Ok(())
        }
    }
}
