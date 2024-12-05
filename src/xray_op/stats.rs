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

use super::client::XrayClients;
use super::users::UserState;
use crate::xray_api::xray::app::stats::command::GetStatsRequest;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;
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
    stat: StatType,
) -> Result<GetStatsResponse, Status> {
    let mut client = clients.stats_client.lock().await;
    let stat_name = format!("user>>>{user_id}@{tag}>>>traffic>>>{stat}");
    debug!("Sending stat request: {}", stat_name);

    let request = Request::new(GetStatsRequest {
        name: stat_name,
        reset: false,
    });

    let response = client.get_stats(request).await?;

    Ok(response.into_inner())
}

pub async fn get_stats_task(
    clients: XrayClients,
    state: Arc<Mutex<UserState>>,
    tag: Tag,
    stat_type: StatType,
) {
    let user_state = state.lock().await;
    loop {
        for user in &user_state.users.clone() {
            match tag {
                Tag::Vmess => {
                    match get_user_stats(
                        clients.clone(),
                        user.user_id.clone(),
                        tag.clone(),
                        stat_type.clone(),
                    )
                    .await
                    {
                        Ok(response) => {
                            info!("{tag} {stat_type} Received stats: {:?}", response);
                        }
                        Err(e) => {
                            error!("{tag} {stat_type} Failed to get stats: {}", e);
                        }
                    }
                }
                Tag::Vless => debug!("{tag} Stat: Not impemented"),
                Tag::Shadowsocks => debug!("{tag} Stat: Not implemented"),
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
}
