use chrono::{Duration, Utc};
use log::{debug, error};
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;

use super::xray_op::{
    client::XrayClients, remove_user, shadowsocks, stats::get_traffic_stats, stats::StatType,
    vless, vmess, Tag,
};

use super::{user::UserStatus, user_state::UserState};

pub async fn sync_state(
    state: Arc<Mutex<UserState>>,
    clients: XrayClients,
    tag: Tag,
) -> Result<(), Box<dyn Error>> {
    let state = state.lock().await;
    let users = &state.users;

    for (user_id, user) in users.iter() {
        debug!("Running sync for {:?} {:?}", tag, user);

        if user.has_proto_tag(tag.clone()) {
            if let UserStatus::Active = user.status {
                match tag {
                    Tag::Vmess => {
                        let user_info = vmess::UserInfo::new(user_id.clone());
                        match vmess::add_user(clients.clone(), user_info).await {
                            Ok(()) => debug!("User sync success {:?}", user),
                            Err(e) => error!("User sync fail {:?} {}", user, e),
                        }
                    }
                    Tag::VlessXtls => {
                        let user_info =
                            vless::UserInfo::new(user_id.clone(), vless::UserFlow::Vision);
                        match vless::add_user(clients.clone(), user_info).await {
                            Ok(()) => debug!("User sync success {:?}", user),
                            Err(e) => error!("User sync fail {:?} {}", user, e),
                        }
                    }
                    Tag::VlessGrpc => {
                        let user_info =
                            vless::UserInfo::new(user_id.clone(), vless::UserFlow::Direct);
                        match vless::add_user(clients.clone(), user_info).await {
                            Ok(()) => debug!("User sync success {:?}", user),
                            Err(e) => error!("User sync fail {:?} {}", user, e),
                        }
                    }
                    Tag::Shadowsocks => {
                        let user_info =
                            shadowsocks::UserInfo::new(user_id.clone(), user.password.clone());
                        match shadowsocks::add_user(clients.clone(), user_info).await {
                            Ok(()) => debug!("User sync success {:?}", user),
                            Err(e) => error!("User sync fail {:?} {}", user, e),
                        }
                    }
                }
            } else {
                debug!("User expired, skip to restore");
            }
        }
    }

    Ok(())
}

pub async fn restore_trial_users(state: Arc<Mutex<UserState>>, clients: XrayClients, debug: bool) {
    let trial_users = state.lock().await.get_all_trial_users(UserStatus::Expired);
    let now = Utc::now();

    for (user_id, user) in trial_users {
        let state = state.clone();
        let clients = clients.clone();

        tokio::spawn(async move {
            let mut state = state.lock().await;

            let user_to_restore = if let Some(modified_at) = user.modified_at {
                now.signed_duration_since(modified_at) >= Duration::hours(24)
            } else {
                now.signed_duration_since(user.created_at) >= Duration::hours(24)
            };

            if user_to_restore {
                debug!(
                    "Restoring user {}: checking expiration, modified_at = {:?}, created_at = {:?}",
                    user_id, user.modified_at, user.created_at
                );

                let vmess_restore = {
                    let user_info = vmess::UserInfo::new(user_id.clone());
                    vmess::add_user(clients.clone(), user_info.clone())
                        .await
                        .map(|_| debug!("User restored in VMess: {}", user_info.uuid))
                        .map_err(|e| error!("Failed to restore user in VMess: {}", e))
                };

                let xtls_vless_restore = {
                    let user_info = vless::UserInfo::new(user_id.clone(), vless::UserFlow::Vision);
                    vless::add_user(clients.clone(), user_info.clone())
                        .await
                        .map(|_| debug!("User restored in Vless: {}", user_info.uuid))
                        .map_err(|e| error!("Failed to restore user in VlessXtls: {}", e))
                };

                let grpc_vless_restore = {
                    let user_info = vless::UserInfo::new(user_id.clone(), vless::UserFlow::Direct);
                    vless::add_user(clients.clone(), user_info.clone())
                        .await
                        .map(|_| debug!("User restored in VLess: {}", user_info.uuid))
                        .map_err(|e| error!("Failed to restore user in VlessGrpc: {}", e))
                };

                if vmess_restore.is_ok() && xtls_vless_restore.is_ok() && grpc_vless_restore.is_ok()
                {
                    if let Err(e) = state.restore_user(&user_id.clone()).await {
                        error!("Failed to update user state: {:?}", e);
                    } else {
                        debug!("Successfully restored user in state: {}", user_id);
                    }
                }
                if debug {
                    let _ = state.save_to_file_async("Restore job").await;
                }
            }
        });
    }
}

pub async fn block_trial_users_by_limit(
    state: Arc<Mutex<UserState>>,
    clients: XrayClients,
    debug: bool,
) {
    let trial_users = state.lock().await.get_all_trial_users(UserStatus::Active);

    for (user_id, user) in trial_users {
        let state = state.clone();
        let user_id = user_id.clone();
        let clients = clients.clone();

        tokio::spawn(async move {
            let mut state = state.lock().await;

            let user_exceeds_limit = user
                .downlink
                .map_or(false, |downlink| downlink > user.limit);

            if user_exceeds_limit {
                debug!(
                    "User {} exceeds the limit: downlink={} > limit={}",
                    user_id,
                    user.downlink.unwrap_or(0),
                    user.limit
                );

                let vmess_remove = remove_user(clients.clone(), user_id.clone(), Tag::Vmess);
                let ss_remove = remove_user(clients.clone(), user_id.clone(), Tag::Shadowsocks);
                let xtls_vless_remove =
                    remove_user(clients.clone(), user_id.clone(), Tag::VlessXtls);
                let grpc_vless_remove =
                    remove_user(clients.clone(), user_id.clone(), Tag::VlessGrpc);

                let results = tokio::try_join!(
                    vmess_remove,
                    xtls_vless_remove,
                    grpc_vless_remove,
                    ss_remove
                );

                match results {
                    Ok(_) => debug!("Successfully blocked user: {}", user_id),
                    Err(e) => error!("Failed to block user {}: {:?}", user_id, e),
                }

                let _ = state.reset_user_stat(&user_id, StatType::Uplink);
                let _ = state.reset_user_stat(&user_id, StatType::Downlink);
                let _ = get_traffic_stats(
                    clients.clone(),
                    format!("user>>>{}@pony>>>traffic", user_id),
                    true,
                );

                if let Err(e) = state.expire_user(&user_id).await {
                    error!("Failed to update status for user {}: {:?}", user_id, e);
                }
                if debug {
                    let _ = state.save_to_file_async("Block by limit job").await;
                }
            }
        });
    }
}
