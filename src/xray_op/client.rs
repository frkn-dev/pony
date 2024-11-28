use crate::config2::Settings;
use crate::xray_api::xray::app::proxyman::command::handler_service_client::HandlerServiceClient;
use crate::xray_api::xray::app::stats::command::stats_service_client::StatsServiceClient;
use crate::xray_api::xray::app::stats::command::GetStatsRequest;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Request;

#[derive(Clone)]
pub struct XrayClients {
    pub handler_client: Arc<Mutex<HandlerServiceClient<Channel>>>,
    pub stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
}

pub async fn create_clients(settings: Settings) -> Result<XrayClients, Box<dyn std::error::Error>> {
    let channel = Channel::from_shared(settings.xray.xray_api_endpoint)
        .unwrap()
        .connect()
        .await?;

    let handler_client = Arc::new(Mutex::new(HandlerServiceClient::new(channel.clone())));

    let stats_client = Arc::new(Mutex::new(StatsServiceClient::new(channel)));

    Ok(XrayClients {
        handler_client,
        stats_client,
    })
}
