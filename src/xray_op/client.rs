use log::error;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use crate::xray_api::xray::app::{
    proxyman::command::handler_service_client::HandlerServiceClient,
    stats::command::stats_service_client::StatsServiceClient,
};

#[derive(Clone)]
pub struct XrayClients {
    pub handler_client: Arc<Mutex<HandlerServiceClient<Channel>>>,
    pub stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
}

impl XrayClients {
    pub async fn new(endpoint: String) -> Result<Self, Box<dyn Error>> {
        let channel = Channel::from_shared(endpoint.clone())?
            .connect()
            .await
            .map_err(|e| {
                error!("Couldn't connect to Xray API at {}: {}", endpoint, e);
                e
            })?;

        let handler_client = Arc::new(Mutex::new(HandlerServiceClient::new(channel.clone())));
        let stats_client = Arc::new(Mutex::new(StatsServiceClient::new(channel)));

        Ok(Self {
            handler_client: handler_client,
            stats_client: stats_client,
        })
    }
}
