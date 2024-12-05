use log::debug;
use log::error;
use log::info;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::time::Duration;
use tonic::Request;
use tonic::Status;

use crate::users::UserState;
use crate::xray_api::xray::app::stats::command::GetStatsRequest;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;
use crate::xray_op::client::XrayClients;
use crate::xray_op::users;
use crate::zmq::Tag;

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

pub async fn get_user_stats(
    clients: XrayClients,
    user_id: String,
    tag: Tag,
) -> Result<(GetStatsResponse, GetStatsResponse), Status> {
    let client = clients.stats_client.lock().await;

    let downlink_stat_name = format!("user>>>{user_id}@{tag}>>>traffic>>>downlink");
    let uplink_stat_name = format!("user>>>{user_id}@{tag}>>>traffic>>>uplink");

    let downlink_request = Request::new(GetStatsRequest {
        name: downlink_stat_name,
        reset: false,
    });
    let uplink_request = Request::new(GetStatsRequest {
        name: uplink_stat_name,
        reset: false,
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
        (Err(e), _) | (_, Err(e)) => {
            error!("Stat request failed: {}", e);
            Err(Status::internal(format!("Stat request failed: {}", e)))
        }
    }
}

pub async fn get_stats_task(clients: XrayClients, state: Arc<Mutex<UserState>>, tag: Tag) {
    loop {
        let mut user_state = state.lock().await;
        for user in user_state.users.clone() {
            match tag {
                Tag::Vmess => {
                    match get_user_stats(clients.clone(), user.user_id.clone(), tag.clone()).await {
                        Ok(response) => {
                            info!("{tag} Received stats: {:?}", response);

                            if let Some(downlink) = response.0.stat {
                                user_state.update_user_downlink(&user.user_id, downlink.value);

                                let _ = users::check_and_block_user(
                                    clients.clone(),
                                    state.clone(),
                                    &user.user_id,
                                    tag.clone(),
                                )
                                .await;
                            }
                            if let Some(uplink) = response.1.stat {
                                user_state.update_user_uplink(&user.user_id, uplink.value);
                            }
                        }
                        Err(e) => {
                            error!("{tag} Failed to get stats: {}", e);
                        }
                    }
                }
                Tag::Vless => debug!("{tag} Stat: Not implemented"),
                Tag::Shadowsocks => debug!("{tag} Stat: Not implemented"),
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
}
