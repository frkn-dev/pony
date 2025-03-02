use log::error;
use std::error::Error;
use tonic::transport::Channel;

use crate::xray_api::xray::app::{
    proxyman::command::handler_service_client::HandlerServiceClient,
    stats::command::stats_service_client::StatsServiceClient,
};

pub trait XrayClient {
    type Client;

    fn new(
        endpoint: &str,
    ) -> impl std::future::Future<Output = Result<Self, Box<dyn Error>>> + Send
    where
        Self: Sized;
}

#[derive(Clone)]
pub struct HandlerClient {
    pub client: HandlerServiceClient<Channel>,
}

impl XrayClient for HandlerClient {
    type Client = HandlerServiceClient<Channel>;

    async fn new(endpoint: &str) -> Result<Self, Box<dyn Error>> {
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await
            .map_err(|e| {
                error!("Couldn't connect to Xray API at {}: {}", endpoint, e);
                e
            })?;

        Ok(Self {
            client: HandlerServiceClient::new(channel),
        })
    }
}

#[derive(Clone)]
pub struct StatsClient {
    pub client: StatsServiceClient<Channel>,
}

impl XrayClient for StatsClient {
    type Client = StatsServiceClient<Channel>;
    async fn new(endpoint: &str) -> Result<Self, Box<dyn Error>> {
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await
            .map_err(|e| {
                error!("Couldn't connect to Xray API at {}: {}", endpoint, e);
                e
            })?;

        Ok(Self {
            client: StatsServiceClient::new(channel),
        })
    }
}
