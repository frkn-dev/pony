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
use crate::xray_op::Tag;
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
    async fn create(&self, conn_id: &uuid::Uuid, tag: Tag, password: Option<String>) -> Result<()>;
    async fn remove(&self, conn_id: &uuid::Uuid, tag: Tag, password: Option<String>) -> Result<()>;
}

#[async_trait::async_trait]
impl HandlerActions for Arc<Mutex<HandlerClient>> {
    async fn create(&self, conn_id: &uuid::Uuid, tag: Tag, password: Option<String>) -> Result<()> {
        match tag {
            Tag::VlessXtls => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Vision);
                let _ = user_info.create(self.clone()).await;
                Ok(())
            }
            Tag::VlessGrpc => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Direct);
                let _ = user_info.create(self.clone()).await;
                Ok(())
            }
            Tag::Vmess => {
                let user_info = VmessConnInfo::new(conn_id);
                let _ = user_info.create(self.clone()).await;
                Ok(())
            }
            Tag::Shadowsocks => {
                if let Some(pass) = password.clone() {
                    let user_info = SsConnInfo::new(conn_id, Some(pass));
                    let _ = user_info.create(self.clone()).await;
                    return Ok(());
                }
                Err(crate::PonyError::Custom(
                    "Create SS user error, password not provided".to_string(),
                ))
            }
        }
    }

    async fn remove(&self, conn_id: &uuid::Uuid, tag: Tag, password: Option<String>) -> Result<()> {
        match tag {
            Tag::VlessXtls => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Vision);
                let _ = user_info.remove(self.clone()).await;
                Ok(())
            }
            Tag::VlessGrpc => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Direct);
                let _ = user_info.remove(self.clone()).await;
                Ok(())
            }
            Tag::Vmess => {
                let user_info = VmessConnInfo::new(conn_id);
                let _ = user_info.remove(self.clone()).await;
                Ok(())
            }
            Tag::Shadowsocks => {
                if let Some(pass) = password.clone() {
                    let user_info = SsConnInfo::new(conn_id, Some(pass));
                    let _ = user_info.remove(self.clone()).await;
                    return Ok(());
                }
                Err(crate::PonyError::Custom(
                    "Remove SS user error, password not provided".to_string(),
                ))
            }
        }
    }
}
