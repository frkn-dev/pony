use async_trait::async_trait;
use tonic::{Code, Request, Status};

use pony::memory::connection::stat::Stat as ConnectionStat;
use pony::memory::node::Stat as InboundStat;
use pony::memory::stat::Kind as StatKind;
use pony::memory::stat::Stat;
use pony::xray_api::xray::app::stats::command::{GetStatsRequest, GetStatsResponse};
use pony::xray_op::connections::ConnOp;
use pony::xray_op::stats::Prefix;
use pony::xray_op::stats::StatOp;
use pony::ConnectionBaseOp;
use pony::Tag;

use super::Agent;

#[async_trait]
impl<C> StatOp for Agent<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    async fn stat(
        &self,
        prefix: Prefix,
        stat_type: Stat,
        reset: bool,
    ) -> Result<GetStatsResponse, Status> {
        if let Some(client) = &self.xray_stats_client {
            let mut stats_client = client.lock().await;

            let base_name = match prefix {
                Prefix::InboundPrefix(tag) => format!("inbound>>>{}", tag),
                Prefix::ConnPrefix(uuid) => format!("user>>>{}@pony", uuid),
            };

            let stat_name = match &stat_type {
                Stat::Conn(StatKind::Downlink)
                | Stat::Conn(StatKind::Uplink)
                | Stat::Inbound(StatKind::Downlink)
                | Stat::Inbound(StatKind::Uplink) => {
                    format!("{}>>>traffic>>>{}", base_name, stat_type)
                }
                Stat::Conn(StatKind::Online) => {
                    format!("{}>>>{}", base_name, stat_type)
                }
                _ => return Err(Status::internal("Unsupported stat type")),
            };

            let request = Request::new(GetStatsRequest {
                name: stat_name,
                reset,
            });

            let response = match stat_type {
                Stat::Conn(StatKind::Online) => stats_client.client.get_stats_online(request).await,
                _ => stats_client.client.get_stats(request).await,
            };

            match response {
                Ok(stat) => Ok(stat.into_inner()),
                Err(e) => Err(Status::internal(format!("Stat request failed: {}", e))),
            }
        } else {
            Err(Status::new(
                Code::Unavailable,
                "Xray Stats client doesn't exist",
            ))
        }
    }

    async fn reset_stat(&self, conn_id: &uuid::Uuid) -> Result<(), Status> {
        let id = Prefix::ConnPrefix(*conn_id);
        let _ = tokio::join!(
            self.stat(id, Stat::Conn(StatKind::Downlink), true),
            self.stat(id, Stat::Conn(StatKind::Uplink), true),
            self.stat(id, Stat::Conn(StatKind::Online), true)
        );
        Ok(())
    }

    async fn conn_stats(&self, conn_id: Prefix) -> Result<ConnectionStat, Status> {
        let (down_res, up_res, online_res) = tokio::join!(
            self.stat(conn_id, Stat::Conn(StatKind::Downlink), false),
            self.stat(conn_id, Stat::Conn(StatKind::Uplink), false),
            self.stat(conn_id, Stat::Conn(StatKind::Online), false)
        );

        if let Err(e) = &down_res {
            log::warn!("Downlink stat missing for {:?}: {}", conn_id, e);
        }
        if let Err(e) = &up_res {
            log::warn!("Uplink stat missing for {:?}: {}", conn_id, e);
        }
        if let Err(e) = &online_res {
            log::warn!("Online stat missing for {:?}: {}", conn_id, e);
        }

        Ok(ConnectionStat {
            downlink: down_res
                .ok()
                .and_then(|s| s.stat)
                .map(|v| v.value)
                .unwrap_or(0),
            uplink: up_res
                .ok()
                .and_then(|s| s.stat)
                .map(|v| v.value)
                .unwrap_or(0),
            online: online_res
                .ok()
                .and_then(|s| s.stat)
                .map(|v| v.value)
                .unwrap_or(0),
        })
    }

    async fn inbound_stats(&self, inbound: Prefix) -> Result<InboundStat, Status> {
        let (down, up, count) = tokio::join!(
            self.stat(inbound, Stat::Inbound(StatKind::Downlink), false),
            self.stat(inbound, Stat::Inbound(StatKind::Uplink), false),
            self.conn_count(*inbound.as_tag().unwrap())
        );

        match (down, up, count) {
            (Ok(d), Ok(u), Ok(c)) => Ok(InboundStat {
                downlink: d.stat.map(|s| s.value).unwrap_or(0),
                uplink: u.stat.map(|s| s.value).unwrap_or(0),
                conn_count: c.unwrap_or(0),
            }),
            _ => Err(Status::internal("Failed to fetch inbound stats")),
        }
    }

    async fn conn_count(&self, inbound: Tag) -> Result<Option<i64>, Status> {
        if let Some(client) = &self.xray_handler_client {
            let mut handler_client = client.lock().await;
            handler_client
                .conn_count_op(inbound)
                .await
                .map(Some)
                .map_err(|e| Status::internal(e.to_string()))
        } else {
            Err(Status::new(
                Code::Unavailable,
                "Xray Handler client missing",
            ))
        }
    }
}
