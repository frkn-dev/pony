use log::error;
use log::info;
use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::time::Duration;
use tonic::transport::Channel;
use tonic::Request;
use tonic::Status;

use crate::xray_api::xray::app::stats::command::stats_service_client::StatsServiceClient;
use crate::xray_api::xray::app::stats::command::GetStatsRequest;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;
use crate::zmq::Tag;

use super::users::UserState;

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

pub async fn get_stats<T, T2>(
    stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
    user_id: String,
    tag: T,
    stat: T2,
    reset: bool,
) -> Result<GetStatsResponse, Status>
where
    T: Display,
    T2: Display,
{
    let mut client = stats_client.lock().await;
    let stat_name = format!("user>>>{user_id}@{tag}>>>traffic>>>{stat}");
    let request = Request::new(GetStatsRequest {
        name: stat_name,
        reset,
    });

    let response = client.get_stats(request).await?;

    Ok(response.into_inner())
}

pub async fn get_stats_task(
    stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
    reset: bool,
    state: Arc<Mutex<UserState>>,
    tag: Tag,
    stat_type: StatType,
) {
    let user_state = state.lock().await;
    loop {
        for user in &user_state.users.clone() {
            match get_stats(
                stats_client.clone(),
                user.user_id.clone(),
                tag.clone(),
                stat_type.clone(),
                reset,
            )
            .await
            {
                Ok(response) => {
                    info!("Received stats: {:?}", response);
                }
                Err(e) => {
                    error!("Failed to get stats: {}", e);
                }
            }
        }
        sleep(Duration::from_secs(10)).await;
    }
}
