use chrono::{Duration, Utc};
use log::debug;
use log::error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::xray_op::{client::XrayClients, user_state::UserState, users::UserStatus, vmess, Tag};

pub async fn restore_trial_users(state: Arc<Mutex<UserState>>, clients: XrayClients) {
    let trial_users = state.lock().await.get_all_trial_users(UserStatus::Expired);
    let now = Utc::now();

    for user in trial_users {
        let state = state.clone();
        let clients = clients.clone();

        let restore_user_task = async move {
            let mut state = state.lock().await;
            let user_to_restore = if let Some(modified_at) = user.modified_at {
                debug!(
                    "Check user for modified_at {:?} {:?}",
                    user,
                    now.signed_duration_since(modified_at)
                );
                now.signed_duration_since(modified_at) >= Duration::hours(24)
            } else {
                debug!(
                    "Check user for created_at {:?} {:?}",
                    user,
                    now.signed_duration_since(user.created_at)
                );
                now.signed_duration_since(user.created_at) >= Duration::hours(24)
            };

            if user_to_restore {
                let tags = vec![Tag::Vmess, Tag::Vless, Tag::Shadowsocks];
                for tag in &tags {
                    let user_info = vmess::UserInfo::new(user.user_id.clone(), tag.clone());
                    match tag {
                        Tag::Vmess => {
                            match vmess::add_user(clients.clone(), user_info.clone()).await {
                                Ok(()) => {
                                    debug!(
                                        "Success User restore by 24h expire {}",
                                        user_info.uuid.clone()
                                    )
                                }
                                Err(e) => {
                                    error!("Failed User restore by 24h expire {}", e)
                                }
                            }
                        }
                        Tag::Vless => debug!("Vless is not implemented yet"),
                        Tag::Shadowsocks => debug!("Shadowsocks is not implemented yet"),
                    }
                }

                if let Err(e) = state.restore_user(user).await {
                    error!("Failed to restore user: {:?}", e);
                }
            }
        };

        tokio::spawn(restore_user_task);
    }
}

pub async fn block_trial_users_by_limit(state: Arc<Mutex<UserState>>, clients: XrayClients) {
    let trial_users = state.lock().await.get_all_trial_users(UserStatus::Active);

    for user in trial_users {
        let state = state.clone();
        let user_id = user.user_id.clone();
        let clients = clients.clone();

        let block_user_task = async move {
            let mut state = state.lock().await;

            let user_to_block = if let Some(downlink) = user.downlink {
                debug!("Check user for limit {:?}", user);
                downlink > (user.limit * 1024 * 1024)
            } else {
                false
            };

            if user_to_block {
                let tags = vec![Tag::Vmess, Tag::Vless, Tag::Shadowsocks];
                for tag in &tags {
                    match tag {
                        Tag::Vmess => {
                            match vmess::remove_user(clients.clone(), user.user_id.clone()).await {
                                Ok(()) => {
                                    debug!("Success User blocked by limit {}", user.user_id.clone())
                                }
                                Err(e) => error!("Fail User block by limit {}", e),
                            }
                        }
                        Tag::Vless => debug!("Vless is not implemented yet"),
                        Tag::Shadowsocks => debug!("Shadowsocks is not implemented yet"),
                    }
                }

                if let Err(e) = state.expire_user(&user_id).await {
                    error!("Failed to restore user: {:?}", e);
                }
            }
        };

        tokio::spawn(block_user_task);
    }
}
