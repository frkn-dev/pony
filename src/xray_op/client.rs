use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use log::debug;

use crate::{
    appconfig::Settings,
    xray_api::xray::app::{
        proxyman::command::handler_service_client::HandlerServiceClient,
        stats::command::stats_service_client::StatsServiceClient,
    },
};

#[derive(Clone)]
pub struct XrayClients {
    pub handler_client: Arc<Mutex<HandlerServiceClient<Channel>>>,
    pub stats_client: Arc<Mutex<StatsServiceClient<Channel>>>,
}

pub async fn create_clients(settings: Settings) -> Result<XrayClients, Box<dyn std::error::Error>> {
    let channel = match Channel::from_shared(settings.xray.xray_api_endpoint.clone())
        .unwrap()
        .connect()
        .await
    {
        Ok(ch) => {
            debug!("Success - Xray Clients created");
            ch
        }
        Err(e) => panic!(
            "Couldnt' connect to Xray API {} {}",
            settings.xray.xray_api_endpoint, e
        ),
    };

    let handler_client = Arc::new(Mutex::new(HandlerServiceClient::new(channel.clone())));

    let stats_client = Arc::new(Mutex::new(StatsServiceClient::new(channel)));

    Ok(XrayClients {
        handler_client,
        stats_client,
    })
}
