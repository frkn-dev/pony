use crate::metrics::metrics::collect_metrics;
use crate::metrics::metrics::MetricType;
use crate::postgres::UserRow;
use crate::utils::send_to_carbon;
use chrono::{Duration, Utc};
use log::{debug, error, info};
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration as TokioDuration};

use reqwest::Client;

use crate::{actions, settings::Settings, user::User};

use super::xray_op::{
    client::XrayClients, remove_user, stats::get_traffic_stats, stats::StatType, vless, vmess, Tag,
};

use super::{state::State, user::UserStatus};

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

pub async fn save_state_to_file_job(state: Arc<Mutex<State>>, interval_secs: u64) {
    loop {
        sleep(TokioDuration::from_secs(interval_secs)).await;

        let state = state.lock().await;
        if let Err(e) = state.save_to_file_async("save job").await {
            error!("Cannot save state file: {}", e);
        } else {
            debug!("State file saved to file");
        }
    }
}

pub async fn init_state(
    state: Arc<Mutex<State>>,
    settings: Settings,
    clients: XrayClients,
    db_user: UserRow,
) -> Result<(), Box<dyn Error>> {
    debug!("Running sync for {:?} {:?}", db_user.user_id, db_user.trial);

    let user = User::new(
        db_user.trial,
        settings.xray.xray_daily_limit_mb,
        Some(db_user.password.clone()),
    );

    let _ = {
        let mut user_state = state.lock().await;
        match user_state.add_user(db_user.user_id, user.clone()).await {
            Ok(user) => {
                debug!("STATE User added {:?}", user);
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
            info!("Create: User added: {:?}", db_user.user_id);
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
                let _ = get_traffic_stats(
                    clients.clone(),
                    format!("user>>>{}@pony>>>traffic", user_id),
                    true,
                );

                if let Err(e) = state.expire_user(user_id).await {
                    error!("Failed to update status for user {}: {:?}", user_id, e);
                }
            }
        });
    }
}
