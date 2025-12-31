use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use super::shadowsocks::ConnInfo as SsConnInfo;
use super::vless::ConnFlow;
use super::vless::ConnInfo as VlessConnInfo;
use super::vmess::ConnInfo as VmessConnInfo;
use super::ProtocolConn;

use crate::error::{PonyError, Result};
use crate::memory::tag::ProtoTag as Tag;
use crate::xray_api::xray::app::{
    proxyman::command::handler_service_client::HandlerServiceClient,
    stats::command::stats_service_client::StatsServiceClient,
};

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
            Tag::VlessTcpReality => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Vision, Tag::VlessTcpReality);
                let _ = user_info.create(self.clone()).await;
                Ok(())
            }
            Tag::VlessGrpcReality => {
                let user_info =
                    VlessConnInfo::new(conn_id, ConnFlow::Direct, Tag::VlessGrpcReality);
                let _ = user_info.create(self.clone()).await;
                Ok(())
            }
            Tag::VlessXhttpReality => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::None, Tag::VlessXhttpReality);
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
            _ => Err(PonyError::Custom("Not supported by Xray".into())),
        }
    }

    async fn remove(&self, conn_id: &uuid::Uuid, tag: Tag, password: Option<String>) -> Result<()> {
        match tag {
            Tag::VlessTcpReality => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Vision, tag);
                let _ = user_info.remove(self.clone()).await;
                Ok(())
            }
            Tag::VlessGrpcReality => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::Direct, tag);
                let _ = user_info.remove(self.clone()).await;
                Ok(())
            }
            Tag::VlessXhttpReality => {
                let user_info = VlessConnInfo::new(conn_id, ConnFlow::None, tag);
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
            Tag::Wireguard => todo!(),
        }
    }
}
