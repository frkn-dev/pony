use async_trait::async_trait;
use tonic::{Request, Status};

use super::connections::ConnOp;
use crate::state::state::NodeStorage;
use crate::state::stats::ConnStat;
use crate::state::stats::InboundStat;
use crate::state::stats::Stat;
use crate::state::stats::StatType;
use crate::xray_api::xray::app::stats::command::{GetStatsRequest, GetStatsResponse};
use crate::xray_op::Tag;
use crate::Agent;

#[derive(Debug, Clone)]
pub enum Prefix {
    ConnPrefix(uuid::Uuid),
    InboundPrefix(Tag),
}

impl Prefix {
    pub fn as_tag(&self) -> Option<&Tag> {
        if let Prefix::InboundPrefix(tag) = self {
            Some(tag)
        } else {
            None
        }
    }
}

#[async_trait]
pub trait StatOp {
    async fn stat(
        &self,
        prefix: Prefix,
        stat_type: Stat,
        reset: bool,
    ) -> Result<GetStatsResponse, Status>;
    async fn conn_stats(&self, conn_id: Prefix) -> Result<ConnStat, Status>;
    async fn inbound_stats(&self, inbound: Prefix) -> Result<InboundStat, Status>;
    async fn conn_count(&self, inbound: Tag) -> Result<Option<i64>, Status>;
}

#[async_trait]
impl<T: NodeStorage + Send + Sync + Clone> StatOp for Agent<T> {
    async fn stat(
        &self,
        prefix: Prefix,
        stat_type: Stat,
        reset: bool,
    ) -> Result<GetStatsResponse, Status> {
        let mut stats_client = self.xray_stats_client.lock().await;

        let base_name = match prefix {
            Prefix::InboundPrefix(tag) => format!("inbound>>>{}", tag),
            Prefix::ConnPrefix(uuid) => format!("user>>>{}@pony", uuid),
        };

        let stat_name = match &stat_type {
            Stat::Conn(StatType::Downlink)
            | Stat::Conn(StatType::Uplink)
            | Stat::Inbound(StatType::Downlink)
            | Stat::Inbound(StatType::Uplink) => {
                format!("{}>>>traffic>>>{}", base_name, stat_type)
            }
            Stat::Conn(StatType::Online) => {
                format!("{}>>>{}", base_name, stat_type)
            }
            Stat::Inbound(StatType::Online) => {
                return Err(Status::internal("Online is not supported for inbound"));
            }
            Stat::Outbound(_) => {
                return Err(Status::internal("Outbound stat type is not implemented"));
            }
        };

        let request = Request::new(GetStatsRequest {
            name: stat_name,
            reset,
        });

        let response = match stat_type {
            Stat::Conn(StatType::Online) => stats_client.client.get_stats_online(request).await,
            _ => stats_client.client.get_stats(request).await,
        };

        match response {
            Ok(stat) => Ok(stat.into_inner()),
            Err(e) => Err(Status::internal(format!(
                "Stat request failed {:?} {}",
                stat_type, e
            ))),
        }
    }

    async fn conn_stats(&self, conn_id: Prefix) -> Result<ConnStat, Status> {
        log::debug!("conn_stats {:?}", conn_id);

        let (downlink_result, uplink_result, online_result) = tokio::join!(
            self.stat(conn_id.clone(), Stat::Conn(StatType::Downlink), false),
            self.stat(conn_id.clone(), Stat::Conn(StatType::Uplink), false),
            self.stat(conn_id.clone(), Stat::Conn(StatType::Online), false)
        );

        match (downlink_result, uplink_result, online_result) {
            (Ok(downlink), Ok(uplink), Ok(online)) => {
                if let (Some(downlink), Some(uplink), Some(online)) = (
                    downlink.stat.clone(),
                    uplink.stat.clone(),
                    online.stat.clone(),
                ) {
                    log::debug!(
                        "Connection Stats fetched successfully: downlink={:?}, uplink={:?}, online={:?}",
                        downlink.clone(),
                        uplink.clone(),
                        online.clone()
                    );
                    Ok(ConnStat {
                        downlink: downlink.value,
                        uplink: uplink.value,
                        online: online.value,
                    })
                } else {
                    let error_msg = format!(
                        "Incomplete connection stats for {:?}: downlink={:?}, uplink={:?}, online={:?}",
                        conn_id,
                        downlink.stat.clone(),
                        uplink.stat.clone(),
                        online.stat.clone()
                    );
                    Err(Status::internal(error_msg))
                }
            }
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                let error_msg = format!(
                    "Failed to fetch connection stats for {:?}: error={:?}",
                    conn_id, e
                );
                Err(Status::internal(error_msg))
            }
        }
    }

    async fn inbound_stats(&self, inbound: Prefix) -> Result<InboundStat, Status> {
        let downlink_result = self
            .stat(inbound.clone(), Stat::Inbound(StatType::Downlink), false)
            .await;

        let uplink_result = self
            .stat(inbound.clone(), Stat::Inbound(StatType::Uplink), false)
            .await;

        let conn_count_result = self.conn_count(inbound.as_tag().unwrap().clone()).await;

        match (downlink_result, uplink_result, conn_count_result) {
            (Ok(downlink), Ok(uplink), Ok(conn_count)) => {
                if let (Some(downlink), Some(uplink), Some(conn_count)) = (
                    downlink.stat.clone(),
                    uplink.stat.clone(),
                    conn_count.clone(),
                ) {
                    log::debug!(
                    "Node Stats successfully fetched: inbound={:?}, downlink={:?}, uplink={:?}, conn_count={:?} ",
                    inbound, downlink, uplink, conn_count
                );
                    Ok(InboundStat {
                        downlink: downlink.value,
                        uplink: uplink.value,
                        conn_count: conn_count,
                    })
                } else {
                    let error_msg = format!(
                    "Incomplete stats for inbound {:?}: downlink={:?}, uplink={:?}, conn_count={:?}",
                    inbound,
                    downlink.stat.clone(),
                    uplink.stat.clone(),
                    conn_count.clone(),
                );
                    Err(Status::internal(error_msg))
                }
            }

            (Err(e1), Err(e2), Err(e3)) => {
                let error_msg = format!(
                "ALl requests failed for inbound {:?}: downlink error: {:?}, uplink error: {:?}, conn_count: {:?}",
                inbound, e1, e2, e3
            );
                Err(Status::internal(error_msg))
            }
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                let error_msg = format!(
                    "One of the requests failed for inbound {:?}: {:?}",
                    inbound, e
                );
                Err(Status::internal(error_msg))
            }
        }
    }

    async fn conn_count(&self, inbound: Tag) -> Result<Option<i64>, Status> {
        let mut handler_client = self.xray_handler_client.lock().await;
        match handler_client.conn_count_op(inbound.clone()).await {
            Ok(count) => Ok(Some(count)),
            Err(e) => Err(Status::internal(format!(
                "Failed to fetch conn count for inbound {}: {}",
                inbound, e
            ))),
        }
    }
}
