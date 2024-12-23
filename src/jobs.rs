use crate::metrics::metrics::collect_metrics;
use crate::metrics::metrics::MetricType;
use crate::postgres::UserRow;
use crate::utils::send_to_carbon;
use crate::xray_op::stats;
use chrono::{Duration, Utc};
use log::{debug, error};
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;

use reqwest::Client;

use crate::{actions, settings::Settings, user::User};

use super::xray_op::{
    client::XrayClients, remove_user, stats::Prefix, stats::StatType, vless, vmess, Tag,
};

use super::{state::State, user::UserStatus};

pub async fn collect_stats_job(
    clients: XrayClients,
    state: Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error>> {
    debug!("Stat job");

    let mut tasks = vec![];

    {
        let state_guard = state.lock().await;
        let user_ids: Vec<_> = state_guard.users.keys().cloned().collect();

        for user_id in user_ids {
            let clients = clients.clone();
            let state = state.clone();

            tasks.push(tokio::spawn(async move {
                if let Ok(user_stat) =
                    stats::get_user_stats(clients.clone(), Prefix::UserPrefix(user_id)).await
                {
                    debug!(
                        "User {} downlink {} uplink {} online {}",
                        user_id, user_stat.downlink, user_stat.uplink, user_stat.online
                    );

                    let mut state_guard = state.lock().await;
                    if let Err(e) = state_guard
                        .update_user_downlink(user_id, user_stat.downlink)
                        .await
                    {
                        error!("Failed to update user downlink: {}", e);
                    }
                    if let Err(e) = state_guard
                        .update_user_uplink(user_id, user_stat.uplink)
                        .await
                    {
                        error!("Failed to update user uplink: {}", e);
                    }
                    if let Err(e) = state_guard
                        .update_user_online(user_id, user_stat.online)
                        .await
                    {
                        error!("Failed to update user online: {}", e);
                    }
                } else {
                    error!("Failed to fetch user stats for {}", user_id);
                }
            }));
        }

        let tags: Vec<_> = state_guard.node.inbounds.keys().cloned().collect();
        for tag in tags {
            let clients = clients.clone();
            let state = state.clone();

            tasks.push(tokio::spawn(async move {
                if let Ok(inbound_stat) =
                    stats::get_inbound_stats(clients.clone(), Prefix::InboundPrefix(tag.clone()))
                        .await
                {
                    debug!(
                        "Node {} downlink {} uplink {}",
                        tag, inbound_stat.downlink, inbound_stat.uplink
                    );
                    let mut state_guard = state.lock().await;
                    if let Err(e) = state_guard
                        .update_node_downlink(tag.clone(), inbound_stat.downlink)
                        .await
                    {
                        error!("Failed to update inbound downlink: {}", e);
                    }
                    if let Err(e) = state_guard
                        .update_node_uplink(tag.clone(), inbound_stat.uplink)
                        .await
                    {
                        error!("Failed to update inbound uplink: {}", e);
                    }
                }
                if let Ok(user_count) = stats::get_user_count(clients.clone(), tag.clone()).await {
                    let mut state_guard = state.lock().await;
                    let _ = state_guard.update_node_user_count(tag.clone(), user_count);
                }
            }))
        }
    }

    let results = futures::future::join_all(tasks).await;

    for result in results {
        if let Err(e) = result {
            error!("Task panicked: {:?}", e);
        }
    }

    Ok(())
}

pub async fn collect_stats_job2(
    clients: XrayClients,
    state: Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error>> {
    let state = state.lock().await;

    debug!("Stat job");

    // Getting user stat: downlink, uplink, online
    let users_tasks: Vec<_> = state
        .users
        .keys()
        .map(|user_id| {
            let clients = clients.clone();
            let mut state = state.clone();
            let user_id = *user_id;

            tokio::spawn(async move {
                if let Ok(user_stat) =
                    stats::get_user_stats(clients.clone(), Prefix::UserPrefix(user_id)).await
                {
                    let _ = {
                        debug!(
                            "User {} downlink {} uplink {} online {}",
                            user_id,
                            user_stat.downlink.clone(),
                            user_stat.uplink.clone(),
                            user_stat.online.clone()
                        );
                        let _ = state
                            .update_user_downlink(user_id, user_stat.downlink)
                            .await;
                        let _ = state.update_user_uplink(user_id, user_stat.uplink).await;
                        let _ = state.update_user_online(user_id, user_stat.online).await;
                    };
                }
            })
        })
        .collect();

    // Getting node stat
    let inbound_tasks: Vec<_> = state
        .node
        .inbounds
        .keys()
        .map(|inbound| {
            let clients = clients.clone();
            let mut state = state.clone();
            let inbound = inbound.clone();

            tokio::spawn({
                async move {
                    // downlink, uplink
                    if let Ok(node_stats) = stats::get_inbound_stats(
                        clients.clone(),
                        Prefix::InboundPrefix(inbound.clone()),
                    )
                    .await
                    {
                        let _ = {
                            debug!(
                                "Node downlink {} uplink {}",
                                node_stats.downlink.clone(),
                                node_stats.uplink.clone()
                            );

                            let _ = state
                                .update_node_downlink(inbound.clone(), node_stats.downlink)
                                .await;
                            let _ = state
                                .update_node_uplink(inbound.clone(), node_stats.uplink)
                                .await;
                        };
                    } else {
                        error!("Cannot get inbound stats");
                    }

                    // user_count
                    if let Ok(user_count) =
                        stats::get_user_count(clients.clone(), inbound.clone()).await
                    {
                        let _ = state.update_node_user_count(inbound.clone(), user_count);
                    }
                }
            })
        })
        .collect();

    let mut tasks = vec![];
    tasks.extend(users_tasks);
    tasks.extend(inbound_tasks);

    let results = futures::future::join_all(tasks).await;

    for result in results {
        if let Err(e) = result {
            error!("Task panicked: {:?}", e);
        }
    }

    Ok(())
}

pub async fn send_metrics_job<T>(
    state: Arc<Mutex<State>>,
    settings: Settings,
) -> Result<(), Box<dyn Error>> {
    let hostname = settings.node.hostname.unwrap_or("localhost".to_string());
    let interface = settings
        .node
        .default_interface
        .unwrap_or("eth0".to_string());
    let metrics =
        collect_metrics::<T>(state.clone(), &settings.node.env, &hostname, &interface).await;

    debug!("Metrics job run");
    for metric in metrics {
        match metric {
            MetricType::F32(m) => send_to_carbon(&m, &settings.carbon.address).await?,
            MetricType::F64(m) => send_to_carbon(&m, &settings.carbon.address).await?,
            MetricType::I64(m) => send_to_carbon(&m, &settings.carbon.address).await?,
            MetricType::U64(m) => send_to_carbon(&m, &settings.carbon.address).await?,
            MetricType::U8(m) => send_to_carbon(&m, &settings.carbon.address).await?,
        }
    }
    Ok(())
}

pub async fn register_node(
    state: Arc<Mutex<State>>,
    settings: Settings,
) -> Result<(), Box<dyn Error>> {
    let node_state = state.lock().await;
    let node = node_state.node.clone();

    debug!("node {:?} ", node.uuid);
    let client = Client::new();

    let endpoint = format!("{}/node/register", settings.api.endpoint_address);
    let res = client.post(endpoint).json(&node).send().await?;

    if res.status().is_success() {
        debug!("Req success!");
    } else {
        error!("Req error: {}", res.status());
    }

    Ok(())
}

pub async fn init_state(
    state: Arc<Mutex<State>>,
    settings: Settings,
    clients: XrayClients,
    db_user: UserRow,
    debug: bool,
) -> Result<(), Box<dyn Error>> {
    debug!("Running sync for {:?} {:?}", db_user.user_id, db_user.trial);

    let user = User::new(
        db_user.trial,
        settings.xray.xray_daily_limit_mb,
        Some(db_user.password.clone()),
    );

    let _ = {
        let mut user_state = state.lock().await;
        match user_state
            .add_user(db_user.user_id, user.clone(), debug)
            .await
        {
            Ok(user) => {
                debug!("User added to State {:?}", user);
            }
            Err(e) => {
                return Err(format!(
                    "Create: Failed to add user {} to state: {}",
                    db_user.user_id, e
                )
                .into());
            }
        }
    };

    match actions::create_users(
        db_user.user_id,
        Some(db_user.password.clone()),
        clients.clone(),
        state.clone(),
    )
    .await
    {
        Ok(_) => {
            debug!("Create: User added: {:?}", db_user.user_id);
            return Ok(());
        }
        Err(_e) => {
            let mut user_state = state.lock().await;
            let _ = user_state.remove_user(db_user.user_id).await;
            return Err(format!("Create: Failed to add user {} to state", db_user.user_id).into());
        }
    }
}

pub async fn restore_trial_users(state: Arc<Mutex<State>>, clients: XrayClients) {
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
                    let user_info = vmess::UserInfo::new(user_id);
                    vmess::add_user(clients.clone(), user_info.clone())
                        .await
                        .map(|_| debug!("User restored in VMess: {}", user_info.uuid))
                        .map_err(|e| error!("Failed to restore user in VMess: {}", e))
                };

                let xtls_vless_restore = {
                    let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Vision);
                    vless::add_user(clients.clone(), user_info.clone())
                        .await
                        .map(|_| debug!("User restored in Vless: {}", user_info.uuid))
                        .map_err(|e| error!("Failed to restore user in VlessXtls: {}", e))
                };

                let grpc_vless_restore = {
                    let user_info = vless::UserInfo::new(user_id, vless::UserFlow::Direct);
                    vless::add_user(clients.clone(), user_info.clone())
                        .await
                        .map(|_| debug!("User restored in VLess: {}", user_info.uuid))
                        .map_err(|e| error!("Failed to restore user in VlessGrpc: {}", e))
                };

                if vmess_restore.is_ok() && xtls_vless_restore.is_ok() && grpc_vless_restore.is_ok()
                {
                    if let Err(e) = state.restore_user(user_id).await {
                        error!("Failed to update user state: {:?}", e);
                    } else {
                        debug!("Successfully restored user in state: {}", user_id);
                    }
                }
            }
        });
    }
}

pub async fn block_trial_users_by_limit(state: Arc<Mutex<State>>, clients: XrayClients) {
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

                let _ = state.reset_user_stat(user_id, StatType::Downlink);

                if let Err(e) = state.expire_user(user_id).await {
                    error!("Failed to update status for user {}: {:?}", user_id, e);
                }
            }
        });
    }
}
