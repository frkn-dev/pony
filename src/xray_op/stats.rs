use log::{debug, info, warn};
use std::{fmt, sync::Arc};
use tokio::{sync::Mutex, time::Duration};
use tonic::{Request, Status};

use super::{client::XrayClients, user, user_state::UserState};
use crate::xray_api::xray::app::stats::command::{GetStatsRequest, GetStatsResponse};

#[derive(Debug, Clone)]
pub enum StatType {
    Uplink,
    Downlink,
}

impl fmt::Display for StatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatType::Uplink => write!(f, "uplink"),
            StatType::Downlink => write!(f, "downlink"),
        }
    }
}

pub async fn stats_task(clients: XrayClients, state: Arc<Mutex<UserState>>) {
    debug!("Stats task running");
    loop {
        let user_state = state.lock().await;
        let users = user_state.users.clone();
        drop(user_state);
        for user in users {
            match get_traffic_stats(
                clients.clone(),
                format!("user>>>{}@pony>>>traffic", user.user_id),
                false,
            )
            .await
            {
                Ok(response) => {
                    info!("Received stats: {:?}", response);

                    let mut user_state = state.lock().await;

                    if let Some(downlink) = response.0.stat {
                        let _ = user_state
                            .update_user_downlink(&user.user_id, downlink.value / (1024 * 1024))
                            .await;
                    }
                    if let Some(uplink) = response.1.stat {
                        let _ = user_state
                            .update_user_uplink(&user.user_id, uplink.value / (1024 * 1024))
                            .await;
                    }
                    let _ = user_state.save_to_file_async("Stats task").await;
                }
                Err(e) => {
                    warn!("Failed to get stats: {}", e);
                }
            }
        }

        let _ = {
            let user_state = state.lock().await;
            let node = user_state.node.clone();
            drop(user_state);

            for inbound in node.inbounds.keys() {
                let mut user_state = state.lock().await;

                match user::user_count(clients.clone(), inbound.clone()).await {
                    Ok(count) => {
                        let _ = user_state
                            .update_node_user_count(inbound.clone(), count)
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to fetch user count for tag {}: {}", inbound, e);
                    }
                }
            }
        };

        let user_state = state.lock().await;
        let node = user_state.node.clone();
        drop(user_state);
        for inbound in node.inbounds.keys() {
            match get_traffic_stats(
                clients.clone(),
                format!("inbound>>>{inbound}>>>traffic"),
                false,
            )
            .await
            {
                Ok(response) => {
                    info!("Received node stats: {:?}", response);

                    let mut user_state = state.lock().await;

                    if let Some(downlink) = response.0.stat {
                        let _ = user_state
                            .update_node_downlink(inbound.clone(), downlink.value)
                            .await;
                    }
                    if let Some(uplink) = response.1.stat {
                        let _ = user_state
                            .update_node_uplink(inbound.clone(), uplink.value)
                            .await;
                    }

                    let _ = user_state.save_to_file_async("Stats task").await;
                }
                Err(e) => {
                    warn!("Failed to get node stats: {}", e);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

pub async fn get_traffic_stats(
    clients: XrayClients,
    stat: String,
    reset: bool,
) -> Result<(GetStatsResponse, GetStatsResponse), Status> {
    let client = clients.stats_client.lock().await;

    let downlink_stat_name = format!("{stat}>>>{}", StatType::Downlink);
    let uplink_stat_name = format!("{stat}>>>{}", StatType::Uplink);

    let downlink_request = Request::new(GetStatsRequest {
        name: downlink_stat_name,
        reset: reset,
    });

    let uplink_request = Request::new(GetStatsRequest {
        name: uplink_stat_name,
        reset: reset,
    });

    let downlink_response = tokio::spawn({
        let mut client = client.clone();
        async move { client.get_stats(downlink_request).await }
    });

    let uplink_response = tokio::spawn({
        let mut client = client.clone();
        async move { client.get_stats(uplink_request).await }
    });

    let (downlink_result, uplink_result) = tokio::try_join!(downlink_response, uplink_response)
        .map_err(|e| Status::internal(format!("Join error: {}", e)))?;

    match (downlink_result, uplink_result) {
        (Ok(downlink), Ok(uplink)) => {
            debug!("Downlink stat: {:?}", downlink);
            debug!("Uplink stat: {:?}", uplink);

            Ok((downlink.into_inner(), uplink.into_inner()))
        }
        (Err(e), _) | (_, Err(e)) => Err(Status::internal(format!("Stat request failed: {}", e))),
    }
}
