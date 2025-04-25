use log::debug;
use log::error;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use uuid::Uuid;

use crate::xray_api::xray::app::{
    proxyman::command::handler_service_client::HandlerServiceClient,
    stats::command::stats_service_client::StatsServiceClient,
};
use crate::xray_op::shadowsocks::UserInfo as SsUserInfo;
use crate::xray_op::vless::UserFlow;
use crate::xray_op::vless::UserInfo as VlessUserInfo;
use crate::xray_op::vmess::UserInfo as VmessUserInfo;
use crate::ProtocolUser;

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

#[async_trait::async_trait]
pub trait HandlerActions {
    async fn create_all(
        &self,
        user_id: uuid::Uuid,
        password: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn remove_all(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait::async_trait]
impl HandlerActions for Arc<Mutex<HandlerClient>> {
    async fn create_all(
        &self,
        user_id: Uuid,
        password: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut protos: Vec<Box<dyn ProtocolUser>> = vec![];

        protos.push(Box::new(VmessUserInfo::new(user_id)));
        protos.push(Box::new(VlessUserInfo::new(user_id, UserFlow::Vision)));
        protos.push(Box::new(VlessUserInfo::new(user_id, UserFlow::Direct)));

        if let Some(pass) = password.clone() {
            protos.push(Box::new(SsUserInfo::new(user_id, Some(pass))));
        }

        for proto in protos {
            if let Err(e) = proto.create(self.clone()).await {
                error!("Failed to create user for tag {:?}: {}", proto.tag(), e);
            } else {
                debug!("Successfully created user for tag {:?}", proto.tag());
            }
        }

        Ok(())
    }

    async fn remove_all(
        &self,
        user_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let protos: Vec<Box<dyn ProtocolUser>> = vec![
            Box::new(VmessUserInfo::new(user_id)),
            Box::new(VlessUserInfo::new(user_id, UserFlow::Vision)),
            Box::new(VlessUserInfo::new(user_id, UserFlow::Direct)),
            Box::new(SsUserInfo::new(user_id, None)),
        ];

        for proto in protos {
            if let Err(e) = proto.remove(self.clone()).await {
                error!("Failed to remove user for tag {:?}: {}", proto.tag(), e);
            } else {
                debug!("Successfully removed user for tag {:?}", proto.tag());
            }
        }

        Ok(())
    }
}
