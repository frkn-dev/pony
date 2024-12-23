use super::Tag;
use log::error;
use log::{debug, warn};
use std::error::Error;
use std::{fmt, sync::Arc};
use tokio::{sync::Mutex, time::Duration};
use tonic::{Request, Status};

use super::{super::state::State, client::XrayClients, user};
use crate::xray_api::xray::app::stats::command::{GetStatsRequest, GetStatsResponse};

#[derive(Debug, Clone)]
pub enum StatType {
    Uplink,
    Downlink,
    Online,
}

impl fmt::Display for StatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatType::Uplink => write!(f, "uplink"),
            StatType::Downlink => write!(f, "downlink"),
            StatType::Online => write!(f, "online"),
        }
    }
}

pub async fn stats_task(clients: XrayClients, state: Arc<Mutex<State>>) {
    debug!("Stats task running");
    loop {
        // User traffic uplink/downlink
        let user_state = state.lock().await;
        let users = user_state.users.clone();
        drop(user_state);
        for user_id in users.keys() {
            match get_traffic_stats(clients.clone(), format!("user>>>{}@pony", user_id), false)
                .await
            {
                Ok(response) => {
                    debug!("Received stats: {:?}", response);

                    let mut user_state = state.lock().await;

                    if let Some(downlink) = response.0.stat {
                        let _ = user_state
                            .update_user_downlink(*user_id, downlink.value / (1024 * 1024))
                            .await;
                    }
                    if let Some(uplink) = response.1.stat {
                        let _ = user_state
                            .update_user_uplink(*user_id, uplink.value / (1024 * 1024))
                            .await;
                    }

                    if let Some(online) = response.2.stat {
                        let _ = user_state.update_user_online(*user_id, online.value).await;
                    }
                }
                Err(e) => {
                    warn!("Failed to get stats: {}", e);
                }
            }
        }

        // Node traffic uplink/downlink
        let _ = {
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
                        debug!("Received node stats: {:?}", response);

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
                    }
                    Err(e) => {
                        warn!("Failed to get node stats: {}", e);
                    }
                }
            }
        };

        // User count per a node inbound
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

        tokio::time::sleep(Duration::from_secs(2000)).await;
    }
}

pub async fn get_traffic_stats(
    clients: XrayClients,
    stat: String,
    reset: bool,
) -> Result<(GetStatsResponse, GetStatsResponse, GetStatsResponse), Status> {
    let client = clients.stats_client.lock().await;

    let downlink_stat_name = format!("{stat}>>>traffic>>>{}", StatType::Downlink);
    let uplink_stat_name = format!("{stat}>>>traffic>>>{}", StatType::Uplink);
    let online_stat_name = format!("{stat}>>>{}", StatType::Online);

    let downlink_request = Request::new(GetStatsRequest {
        name: downlink_stat_name,
        reset: reset,
    });

    let uplink_request = Request::new(GetStatsRequest {
        name: uplink_stat_name,
        reset: reset,
    });

    let online_request = Request::new(GetStatsRequest {
        name: online_stat_name,
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

    let online_response = tokio::spawn({
        let mut client = client.clone();
        async move { client.get_stats_online(online_request).await }
    });

    let (downlink_result, uplink_result, online_result) =
        tokio::try_join!(downlink_response, uplink_response, online_response)
            .map_err(|e| Status::internal(format!("Join error: {}", e)))?;

    match (downlink_result, uplink_result, online_result) {
        (Ok(downlink), Ok(uplink), Ok(online)) => {
            debug!("Downlink stat: {:?}", downlink);
            debug!("Uplink stat: {:?}", uplink);
            debug!("Online stat: {:?}", online);

            Ok((
                downlink.into_inner(),
                uplink.into_inner(),
                online.into_inner(),
            ))
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            Err(Status::internal(format!("Stat request failed: {}", e)))
        }
    }
}
