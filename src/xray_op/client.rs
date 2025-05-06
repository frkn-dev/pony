use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use super::shadowsocks::ConnInfo as SsConnInfo;
use super::vless::ConnFlow;
use super::vless::ConnInfo as VlessConnInfo;
use super::vmess::ConnInfo as VmessConnInfo;
use super::ProtocolConn;
use crate::xray_api::xray::app::{
    proxyman::command::handler_service_client::HandlerServiceClient,
    stats::command::stats_service_client::StatsServiceClient,
};
use crate::Result;

pub trait XrayClient {
    type Client;

    fn new(endpoint: &str) -> impl std::future::Future<Output = Result<Self>> + Send
    where
        Self: Sized;
}

#[derive(Clone)]
pub struct HandlerClient {
    pub client: HandlerServiceClient<Channel>,
}

impl XrayClient for HandlerClient {
    type Client = HandlerServiceClient<Channel>;

    async fn new(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await
            .map_err(|e| {
                log::error!("Couldn't connect to Xray API at {}: {}", endpoint, e);
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
    async fn new(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await
            .map_err(|e| {
                log::error!("Couldn't connect to Xray API at {}: {}", endpoint, e);
                e
            })?;

        Ok(Self {
            client: StatsServiceClient::new(channel),
        })
    }
}

#[async_trait::async_trait]
pub trait HandlerActions {
    async fn create_all(&self, conn_id: &uuid::Uuid, password: Option<String>) -> Result<()>;
    async fn remove_all(&self, conn_id: &uuid::Uuid) -> Result<()>;
}

#[async_trait::async_trait]
impl HandlerActions for Arc<Mutex<HandlerClient>> {
    async fn create_all(&self, conn_id: &uuid::Uuid, password: Option<String>) -> Result<()> {
        let mut protos: Vec<Box<dyn ProtocolConn>> = vec![];

        protos.push(Box::new(VmessConnInfo::new(conn_id)));
        protos.push(Box::new(VlessConnInfo::new(conn_id, ConnFlow::Vision)));
        protos.push(Box::new(VlessConnInfo::new(conn_id, ConnFlow::Direct)));

        if let Some(pass) = password.clone() {
            protos.push(Box::new(SsConnInfo::new(conn_id, Some(pass))));
        }

        for proto in protos {
            if let Err(e) = proto.create(self.clone()).await {
                log::error!(
                    "Failed to create connection for tag {:?}: {}",
                    proto.tag(),
                    e
                );
            } else {
                log::debug!("Successfully created connection for tag {:?}", proto.tag());
            }
        }

        Ok(())
    }

    async fn remove_all(&self, conn_id: &uuid::Uuid) -> Result<()> {
        let protos: Vec<Box<dyn ProtocolConn>> = vec![
            Box::new(VmessConnInfo::new(conn_id)),
            Box::new(VlessConnInfo::new(conn_id, ConnFlow::Vision)),
            Box::new(VlessConnInfo::new(conn_id, ConnFlow::Direct)),
            Box::new(SsConnInfo::new(conn_id, None)),
        ];

        for proto in protos {
            if let Err(e) = proto.remove(self.clone()).await {
                log::error!(
                    "Failed to remove connection for tag {:?}: {}",
                    proto.tag(),
                    e
                );
            } else {
                log::debug!("Successfully removed connection for tag {:?}", proto.tag());
            }
        }

        Ok(())
    }
}
