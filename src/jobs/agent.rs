use chrono::{Duration, Utc};
use log::{debug, error};
use reqwest::Url;
use reqwest::{Client, StatusCode};

use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::settings::AgentSettings;
use crate::metrics::metrics::{collect_metrics, send_to_carbon, MetricType};
use crate::postgres::user::UserRow;
use crate::state::{
    state::State,
    stats::StatType,
    user::{User, UserStatus},
};
use crate::zmq::message::Action;
use crate::zmq::message::Message;

use crate::xray_op::{
    actions::create_users, actions::remove_user, client::XrayClients, stats, stats::Prefix, vless,
    vmess,
};

pub async fn collect_stats_job(
    clients: XrayClients,
    state: Arc<Mutex<State>>,
    node_id: Uuid,
    env: String,
) -> Result<(), Box<dyn Error>> {
    let mut tasks = vec![];

    let user_ids = {
        let state_guard = state.lock().await;
        state_guard.users.keys().cloned().collect::<Vec<_>>()
    };

    for user_id in user_ids {
        let clients = clients.clone();
        let state = state.clone();

        tasks.push(tokio::spawn(async move {
            if let Ok(user_stat) =
                stats::get_user_stats(clients.clone(), Prefix::UserPrefix(user_id)).await
            {
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

    let (node_tags, node_exists) = {
        let state_guard = state.lock().await;
        if let Some(node) = state_guard.get_node(env.clone(), node_id) {
            (node.inbounds.keys().cloned().collect::<Vec<_>>(), true)
        } else {
            (vec![], false)
        }
    };

    if node_exists {
        for tag in node_tags {
            let clients = clients.clone();
            let state = state.clone();
            let env = env.clone();

            tasks.push(tokio::spawn(async move {
                if let Ok(inbound_stat) =
                    stats::get_inbound_stats(clients.clone(), Prefix::InboundPrefix(tag.clone()))
                        .await
                {
                    let mut state_guard = state.lock().await;
                    if let Err(e) = state_guard
                        .update_node_downlink(
                            tag.clone(),
                            inbound_stat.downlink,
                            env.clone(),
                            node_id,
                        )
                        .await
                    {
                        error!("Failed to update inbound downlink: {}", e);
                    }
                    if let Err(e) = state_guard
                        .update_node_uplink(tag.clone(), inbound_stat.uplink, env.clone(), node_id)
                        .await
                    {
                        error!("Failed to update inbound uplink: {}", e);
                    }
                    if let Err(e) = state_guard
                        .update_node_user_count(
                            tag.clone(),
                            inbound_stat.user_count,
                            env.clone(),
                            node_id,
                        )
                        .await
                    {
                        error!("Failed to update user_count: {}", e);
                    }
                }
            }));
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

pub async fn send_metrics_job<T>(
    state: Arc<Mutex<State>>,
    settings: AgentSettings,
    node_id: Uuid,
) -> Result<(), Box<dyn Error>> {
    let hostname = settings.node.hostname.unwrap_or("localhost".to_string());
    let interface = settings
        .node
        .default_interface
        .unwrap_or("eth0".to_string());
    let metrics = collect_metrics::<T>(
        state.clone(),
        &settings.node.env,
        &hostname,
        &interface,
        node_id,
    )
    .await;

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

async fn block_user_req(
    user_id: Uuid,
    env: String,
    endpoint: String,
    token: String,
) -> Result<(), Box<dyn Error>> {
    let msg = Message {
        user_id: user_id,
        action: Action::Delete,
        env: env,
        trial: None,
        limit: None,
        password: None,
    };

    let mut endpoint = Url::parse(&endpoint)?;
    endpoint
        .path_segments_mut()
        .map_err(|_| "Invalid API endpoint")?
        .push("user");
    let endpoint = endpoint.to_string();

    debug!("ENDPOINT: {}", endpoint);

    match serde_json::to_string_pretty(&msg) {
        Ok(json) => debug!("Serialized Message for user '{}': {}", user_id, json),
        Err(e) => error!("Error serializing Message for user_id '{}': {}", user_id, e),
    }

    let res = Client::new()
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .json(&msg)
        .send()
        .await?;

    if res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED {
        return Ok(());
    } else {
        return Err(format!("Req error: {} {:?}", res.status(), res).into());
    }
}

pub async fn register_node(
    state: Arc<Mutex<State>>,
    endpoint: String,
    env: String,
    token: String,
) -> Result<(), Box<dyn Error>> {
    let node_state = state.lock().await;
    let node = node_state.nodes.get(&env).clone();

    let mut endpoint = Url::parse(&endpoint)?;
    endpoint
        .path_segments_mut()
        .map_err(|_| "Invalid API endpoint")?
        .push("node")
        .push("register");
    let endpoint = endpoint.to_string();

    debug!("ENDPOINT: {}", endpoint);

    match serde_json::to_string_pretty(&node) {
        Ok(json) => debug!("Serialized node for environment '{}': {}", env, json),
        Err(e) => error!("Error serializing node for environment '{}': {}", env, e),
    }

    if let Some(env) = node {
        if let Some(node) = env.first() {
            let res = Client::new()
                .post(&endpoint)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .json(&node)
                .send()
                .await?;

            if res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED {
                return Ok(());
            } else {
                return Err(format!("Req error: {} {:?}", res.status(), res).into());
            }
        } else {
            return Err("No nodes found in environment".into());
        }
    } else {
        return Err("No data found for the given environment".into());
    }
}

pub async fn init_state(
    state: Arc<Mutex<State>>,
    limit: i64,
    clients: XrayClients,
    db_user: UserRow,
) -> Result<(), Box<dyn Error>> {
    let user = User::new(
        db_user.trial,
        limit,
        db_user.cluster,
        Some(db_user.password.clone()),
    );

    let _ = {
        let mut user_state = state.lock().await;
        match user_state.add_user(db_user.user_id, user.clone()).await {
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

    match create_users(
        db_user.user_id,
        Some(db_user.password.clone()),
        clients.clone(),
        state.clone(),
    )
    .await
    {
        Ok(_) => {
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
                        state.reset_user_stat(user_id, StatType::Downlink);
                        debug!("Successfully restored user in state: {}", user_id);
                    }
                }
            }
        });
    }
}

pub async fn block_trial_users_by_limit(
    state: Arc<Mutex<State>>,
    clients: XrayClients,
    env: String,
    node_id: Uuid,
    endpoint: String,
    token: String,
) {
    let trial_users = {
        let state_guard = state.lock().await;
        state_guard.get_all_trial_users(UserStatus::Active)
    };

    for (user_id, user) in trial_users {
        let state = state.clone();
        let user_id = user_id.clone();
        let user = user.clone();
        let clients = clients.clone();
        let env = env.clone();
        let endpoint = endpoint.clone();
        let token = token.clone();

        tokio::spawn(async move {
            let user_exceeds_limit = {
                let limit_in_bytes = user.limit * 1_048_576;
                user.downlink
                    .map_or(false, |downlink| downlink > limit_in_bytes)
            };

            if user_exceeds_limit {
                let downlink_mb = user.downlink.unwrap() / 1_048_576;
                debug!(
                    "User {} exceeds the limit: downlink={} > limit={}",
                    user_id, downlink_mb, user.limit
                );

                let inbounds = {
                    let state_guard = state.lock().await;

                    if let Some(node) = state_guard.get_node(env.clone(), node_id) {
                        node.inbounds.keys().cloned().collect::<Vec<_>>()
                    } else {
                        vec![]
                    }
                };

                let remove_tasks: Vec<_> = inbounds
                    .into_iter()
                    .map(|inbound| {
                        tokio::spawn(remove_user(clients.clone(), user_id.clone(), inbound))
                    })
                    .collect();

                if let Some(Err(e)) = futures::future::join_all(remove_tasks)
                    .await
                    .into_iter()
                    .find(|r| r.is_err())
                {
                    error!("Failed to block user {}: {:?}", user_id, e);
                } else {
                    let _ =
                        block_user_req(user_id.clone(), user.env.clone(), endpoint, token).await;
                    debug!("Successfully blocked user: {}", user_id);
                }

                if let Err(e) = {
                    let mut state_guard = state.lock().await;
                    state_guard.expire_user(user_id).await
                } {
                    error!("Failed to update status for user {}: {:?}", user_id, e);
                }
            } else {
                let downlink_mb = user.downlink.unwrap() / 1_048_576;
                debug!(
                    "Check limit: Left free mb {} for user {} {:?}",
                    user.limit - downlink_mb,
                    user_id,
                    user
                );
            }
        });
    }
}
