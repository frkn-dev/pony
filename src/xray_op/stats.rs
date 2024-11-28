use crate::xray_api::xray::app::stats::command::stats_service_client::StatsServiceClient;
use crate::xray_api::xray::app::stats::command::GetStatsRequest;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;
use log::error;
use log::info;
use tokio::time::sleep;
use tokio::time::Duration;

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Request;

pub async fn get_stats(
    stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
    stat_name: String,
    reset: bool,
) -> Result<GetStatsResponse, tonic::Status> {
    let mut client = stats_client.lock().await;

    let request = Request::new(GetStatsRequest {
        name: stat_name,
        reset,
    });

    let response = client.get_stats(request).await?;

    Ok(response.into_inner())
}

pub async fn get_stats_task(
    stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
    stat_name: String,
    reset: bool,
) {
    loop {
        match get_stats(stats_client.clone(), stat_name.clone(), reset).await {
            Ok(response) => {
                info!("Received stats: {:?}", response);
            }
            Err(e) => {
                error!("Failed to get stats: {}", e);
            }
        }

        sleep(Duration::from_secs(10)).await;
    }
}
