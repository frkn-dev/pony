use log::error;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
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

    fn lock(
        &self,
    ) -> impl std::future::Future<Output = tokio::sync::MutexGuard<'_, Self::Client>> + Send;
}

#[derive(Clone)]
pub struct HandlerClient {
    pub client: Arc<Mutex<HandlerServiceClient<Channel>>>,
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
            client: Arc::new(HandlerServiceClient::new(channel).into()),
        })
    }

    async fn lock(&self) -> tokio::sync::MutexGuard<'_, HandlerServiceClient<Channel>> {
        self.client.lock().await
    }
}

#[derive(Clone)]
pub struct StatsClient {
    pub client: Arc<Mutex<StatsServiceClient<Channel>>>,
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
            client: Arc::new(StatsServiceClient::new(channel).into()),
        })
    }

    async fn lock(&self) -> tokio::sync::MutexGuard<'_, StatsServiceClient<Channel>> {
        self.client.lock().await
    }
}
