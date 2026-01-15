use async_trait::async_trait;
use futures::future::join_all;

use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::Code;
use tonic::{Request, Status};

use pony::memory::connection::stat::Stat as ConnectionStat;
use pony::memory::node::Stat as InboundStat;
use pony::memory::stat::Kind as StatKind;
use pony::memory::stat::Stat;
use pony::xray_api::xray::app::stats::command::{GetStatsRequest, GetStatsResponse};
use pony::xray_op::connections::ConnOp;
use pony::xray_op::stats::Prefix;
use pony::xray_op::stats::StatOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStorageBaseOp;
use pony::NodeStorageOp;
use pony::Result as PonyResult;
use pony::SubscriptionOp;
use pony::Tag;

use super::Agent;

#[async_trait]
impl<T, C, S> StatOp for Agent<T, C, S>
where
    T: NodeStorageOp + Send + Sync + Clone,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
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
                Stat::Inbound(StatKind::Online) => {
                    return Err(Status::internal("Online is not supported for inbound"));
                }
                Stat::Outbound(_) => {
                    return Err(Status::internal("Outbound stat type is not implemented"));
                }
                Stat::Conn(StatKind::Unknown) | Stat::Inbound(StatKind::Unknown) => {
                    return Err(Status::internal("Unknown stat type is not implemented"));
                }
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
                Err(e) => Err(Status::internal(format!(
                    "Stat request failed {:?} {}",
                    stat_type, e
                ))),
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
        let (downlink_result, uplink_result, online_result) = tokio::join!(
            self.stat(id, Stat::Conn(StatKind::Downlink), true),
            self.stat(id, Stat::Conn(StatKind::Uplink), true),
            self.stat(id, Stat::Conn(StatKind::Online), true)
        );

        match (downlink_result, uplink_result, online_result) {
            (Ok(downlink), Ok(uplink), Ok(online)) => {
                if let (Some(downlink), Some(uplink), Some(online)) = (
                    downlink.stat.clone(),
                    uplink.stat.clone(),
                    online.stat.clone(),
                ) {
                    log::debug!(
                        "Connection Stats reset successfully: downlink={:?}, uplink={:?}, online={:?}",
                        downlink.clone(),
                        uplink.clone(),
                        online.clone()
                    );

                    Ok(())
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

    async fn conn_stats(&self, conn_id: Prefix) -> Result<ConnectionStat, Status> {
        log::debug!("conn_stats {:?}", conn_id);

        let (downlink_result, uplink_result, online_result) = tokio::join!(
            self.stat(conn_id.clone(), Stat::Conn(StatKind::Downlink), false),
            self.stat(conn_id.clone(), Stat::Conn(StatKind::Uplink), false),
            self.stat(conn_id.clone(), Stat::Conn(StatKind::Online), false)
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
                    Ok(ConnectionStat {
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
            .stat(inbound.clone(), Stat::Inbound(StatKind::Downlink), false)
            .await;

        let uplink_result = self
            .stat(inbound.clone(), Stat::Inbound(StatKind::Uplink), false)
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
        if let Some(client) = &self.xray_handler_client {
            let mut handler_client = client.lock().await;
            match handler_client.conn_count_op(inbound.clone()).await {
                Ok(count) => Ok(Some(count)),
                Err(e) => Err(Status::internal(format!(
                    "Failed to fetch conn count for inbound {}: {}",
                    inbound, e
                ))),
            }
        } else {
            Err(Status::new(
                Code::Unavailable,
                "Xray Hanler client doesn't exist",
            ))
        }
    }
}

impl<T, C, S> Agent<T, C, S>
where
    T: NodeStorageOp + Send + Sync + Clone,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static + std::cmp::PartialEq + SubscriptionOp,
{
    async fn collect_conn_stats(self: Arc<Self>, conn_id: uuid::Uuid) -> PonyResult<()> {
        let conn_stat = self.conn_stats(Prefix::ConnPrefix(conn_id)).await?;
        let mut mem = self.memory.write().await;
        let _ = mem
            .connections
            .update_downlink(&conn_id, conn_stat.downlink);
        let _ = mem.connections.update_uplink(&conn_id, conn_stat.uplink);
        let _ = mem.connections.update_online(&conn_id, conn_stat.online);
        Ok(())
    }

    async fn collect_inbound_stats(
        self: Arc<Self>,
        tag: Tag,
        env: String,
        node_uuid: uuid::Uuid,
    ) -> PonyResult<()> {
        let inbound_stat = self
            .inbound_stats(Prefix::InboundPrefix(tag.clone()))
            .await?;
        let mut mem = self.memory.write().await;
        let _ = mem
            .nodes
            .update_node_downlink(&tag, inbound_stat.downlink, &env, &node_uuid);
        let _ = mem
            .nodes
            .update_node_uplink(&tag, inbound_stat.uplink, &env, &node_uuid);
        let _ = mem
            .nodes
            .update_node_conn_count(&tag, inbound_stat.conn_count, &env, &node_uuid);
        Ok(())
    }

    pub async fn collect_stats(self: Arc<Self>) -> PonyResult<()> {
        log::debug!("Running xray stat job");
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();

        let conn_ids = {
            let mem = self.memory.write().await;
            mem.connections.keys().cloned().collect::<Vec<_>>()
        };

        for conn_id in conn_ids {
            let agent = self.clone();
            tasks.push(tokio::spawn(async move {
                if let Err(e) = agent.collect_conn_stats(conn_id).await {
                    log::error!("Failed to collect stats for connection {}: {}", conn_id, e);
                }
            }));
        }

        if let Some(node) = {
            let mem = self.memory.read().await;
            mem.nodes.get_self()
        } {
            let node_tags = node.inbounds.keys().cloned().collect::<Vec<_>>();
            for tag in node_tags {
                let agent = self.clone();
                let env = node.env.clone();
                let node_uuid = node.uuid;
                tasks.push(tokio::spawn(async move {
                    if let Err(e) = agent
                        .collect_inbound_stats(tag.clone(), env, node_uuid)
                        .await
                    {
                        log::error!("Failed to collect stats for inbound {}: {}", tag.clone(), e);
                    }
                }));
            }
        }

        let results = join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Task panicked: {:?}", e);
            }
        }

        Ok(())
    }

    pub async fn collect_wireguard_stats(self: Arc<Self>) -> PonyResult<()> {
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();

        let wg_client = match &self.wg_client {
            Some(client) => client.clone(),
            None => {
                log::warn!("WG API client is not available, skipping stats collection");
                return Ok(());
            }
        };

        let conns = {
            let mem = self.memory.read().await;
            mem.connections
                .iter()
                .filter_map(|(id, conn)| {
                    if let Tag::Wireguard = conn.get_proto().proto() {
                        Some((*id, conn.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };

        for (conn_id, conn) in conns {
            let wg_client = wg_client.clone();
            let agent = self.clone();
            tasks.push(tokio::spawn(async move {
                if let Some(wg) = conn.get_wireguard() {
                    match wg_client.peer_stats(&wg.keys.pubkey.clone()) {
                        Ok((uplink, downlink)) => {
                            let mut mem = agent.memory.write().await;
                            if let Some(existing) = mem.connections.get_mut(&conn_id) {
                                existing.set_uplink(uplink);
                                existing.set_downlink(downlink);
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to collect WG stats for {} {}: {}",
                                conn_id,
                                wg.keys.pubkey,
                                e
                            );
                        }
                    }
                }
            }));
        }

        let results = join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("WireGuard stats task panicked: {:?}", e);
            }
        }

        Ok(())
    }
}
