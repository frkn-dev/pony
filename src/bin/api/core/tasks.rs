use async_trait::async_trait;
use chrono::Duration;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use pony::ConnectionStorageApiOp;
use std::collections::HashMap;

use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::Action;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::Message;
use pony::NodeStorageOp;
use pony::OperationStatus;
use pony::PonyError;
use pony::Result;
use pony::Tag;

use crate::core::clickhouse::query::Queries;
use crate::core::postgres::connection::ConnRow;
use crate::core::postgres::user::UserRow;
use crate::core::sync::tasks::SyncOp;
use crate::core::sync::SyncTask;
use crate::Api;

#[async_trait]
pub trait Tasks {
    async fn add_node(&self, db_node: Node) -> Result<()>;
    async fn add_conn(&self, db_conn: ConnRow) -> Result<OperationStatus>;
    async fn add_user(&self, db_user: UserRow) -> Result<OperationStatus>;
    async fn node_healthcheck(&self) -> Result<()>;
    async fn collect_conn_stat(&self) -> Result<()>;
    async fn enforce_xray_trial_limits(&self) -> Result<()>;
    async fn restore_xray_trial_conns(&self) -> Result<()>;
}

#[async_trait]
impl<N, C> Tasks for Api<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Send
        + Sync
        + Clone
        + 'static
        + From<Connection>
        + std::cmp::PartialEq,
    Vec<(uuid::Uuid, Connection)>: FromIterator<(uuid::Uuid, C)>,
    Connection: From<C>,
{
    async fn add_user(&self, db_user: UserRow) -> Result<OperationStatus> {
        let user_id = db_user.user_id;
        let user = db_user.try_into()?;

        let mut mem = self.sync.memory.write().await;

        if mem.users.contains_key(&user_id) {
            return Ok(OperationStatus::AlreadyExist(user_id));
        }

        mem.users.insert(user_id, user);
        Ok(OperationStatus::Ok(user_id))
    }

    async fn add_conn(&self, db_conn: ConnRow) -> Result<OperationStatus> {
        let conn_id = db_conn.conn_id;
        let conn: Connection = db_conn.try_into()?;

        let mut state = self.sync.memory.write().await;
        state.connections.add(&conn_id, conn.into()).map_err(|e| {
            format!(
                "Create: Failed to add connection {} to state: {}",
                conn_id, e
            )
            .into()
        })
    }
    async fn add_node(&self, db_node: Node) -> Result<()> {
        let mut state = self.sync.memory.write().await;
        match state.nodes.add(db_node.clone()) {
            Ok(_) => {
                log::debug!("Node added to State: {}", db_node.uuid);
                Ok(())
            }
            Err(e) => Err(format!(
                "Create: Failed to add node {} to state: {}",
                db_node.uuid, e
            )
            .into()),
        }
    }
    async fn node_healthcheck(&self) -> Result<()> {
        let mem = self.sync.memory.read().await;
        let nodes = match mem.nodes.all() {
            Some(n) => n.clone(),
            None => return Ok(()),
        };
        drop(mem);

        let timeout = Duration::seconds(self.settings.api.node_health_check_timeout as i64);
        let now = Utc::now();

        let tasks = nodes.into_iter().map(|node| {
            let ch = self.ch.clone();
            let sync = self.sync.clone();
            let uuid = node.uuid;
            let env = node.env.clone();
            let hostname = node.hostname;

            async move {
                let status =
                    match ch.fetch_node_heartbeat::<f64>(&env, &uuid, &hostname).await {
                        Some(hb) => {
                            let ts = Utc.timestamp_opt(hb.timestamp, 0);
                            match ts {
                                chrono::LocalResult::Single(dt) => {
                                    let diff = now - dt;
                                    if diff <= timeout {
                                        log::debug!(
                            "[ONLINE] Heartbeat OK: now: {}, dt: {}, diff: {}s <= {}s",
                            now, dt, diff.num_seconds(), timeout.num_seconds()
                        );
                                        NodeStatus::Online
                                    } else {
                                        log::debug!(
                            "[OFFLINE] Heartbeat too old: now: {}, dt: {}, diff: {}s > {}s",
                            now, dt, diff.num_seconds(), timeout.num_seconds()
                        );
                                        NodeStatus::Offline
                                    }
                                }
                                _ => {
                                    log::debug!(
                                        "[OFFLINE] Could not parse timestamp from heartbeat: {:?}",
                                        hb.timestamp
                                    );
                                    NodeStatus::Offline
                                }
                            }
                        }
                        None => {
                            log::debug!(
                                "[OFFLINE] No heartbeat data found for node {} ({}) in env {}",
                                uuid,
                                hostname,
                                env
                            );
                            NodeStatus::Offline
                        }
                    };

                SyncOp::update_node_status(&sync, &uuid, &env, status).await?;
                Ok::<_, PonyError>(())
            }
        });

        for result in join_all(tasks).await {
            if let Err(e) = result {
                log::error!("Healthcheck task error: {:?}", e);
            }
        }

        Ok(())
    }

    async fn collect_conn_stat(&self) -> Result<()> {
        let conns_map = {
            let mem = self.sync.memory.read().await;
            mem.connections
                .iter()
                .filter(|(_, conn)| conn.get_status() == ConnectionStatus::Active)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let today = Utc::now().date_naive();
        let start_of_day =
            Utc.from_utc_datetime(&today.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));

        let tasks = conns_map.into_iter().map(|(conn_id, _)| {
            let ch = self.ch.clone();
            let sync = self.sync.clone();

            async move {
                if let Some(metrics) = ch.fetch_conn_stats::<i64>(conn_id, start_of_day).await {
                    log::debug!("metrics {:?}", metrics);
                    let stat = ConnectionStat::from_metrics(metrics);
                    log::debug!("Stat to update - {}", stat);
                    SyncOp::update_conn_stat(&sync, &conn_id, stat).await
                } else {
                    log::warn!("No metrics found for conn_id {}", conn_id);
                    Ok(())
                }
            }
        });

        let results = join_all(tasks).await;

        for result in results {
            if let Err(e) = result {
                log::error!("Error during stat update: {:?}", e);
            }
        }

        Ok(())
    }

    async fn enforce_xray_trial_limits(&self) -> Result<()> {
        let conns: Vec<(uuid::Uuid, Connection)> = {
            let mem = self.sync.memory.read().await;
            mem.connections
                .iter()
                .filter_map(|(id, conn)| {
                    if conn.get_trial() && conn.get_status() == ConnectionStatus::Active {
                        Some((*id, conn.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut grouped: HashMap<Option<uuid::Uuid>, Vec<(uuid::Uuid, Connection)>> =
            HashMap::new();
        for (conn_id, conn) in conns {
            grouped
                .entry(conn.get_user_id())
                .or_default()
                .push((conn_id, conn));
        }

        let sync = self.sync.clone();
        let publisher = self.publisher.clone();

        let tasks = grouped.into_iter().map(move |(_user_id, conns)| {
            let sync = sync.clone();
            let publisher = publisher.clone();

            async move {
                let total_downlink: i64 = conns.iter().map(|(_, c)| c.get_downlink()).sum();

                let limit = conns.iter().map(|(_, c)| c.get_limit()).max().unwrap_or(0);

                if (total_downlink as f64 / 1_048_576.0) >= limit as f64 {
                    for (conn_id, conn) in conns {
                        {
                            let mut mem = sync.memory.write().await;
                            if let Some(conn) = mem.connections.get_mut(&conn_id) {
                                conn.set_status(ConnectionStatus::Expired);
                                conn.set_modified_at();
                            }
                        }

                        let _ = sync
                            .sync_tx
                            .send(SyncTask::UpdateConnStatus {
                                conn_id,
                                status: ConnectionStatus::Expired,
                            })
                            .await;

                        let msg = Message {
                            action: Action::Delete,
                            conn_id,
                            password: conn.get_password(),
                            tag: conn.get_proto().proto(),
                            wg: None,
                        };

                        let _ = publisher.send(&conn.get_env(), msg).await;

                        log::info!(
                            "Trial connection {} expired: downlink = {:.2} MB, limit = {} MB",
                            conn_id,
                            total_downlink as f64 / 1_048_576.0,
                            limit
                        );
                    }
                }

                Ok::<_, PonyError>(())
            }
        });

        let results = join_all(tasks).await;

        for result in results {
            if let Err(e) = result {
                log::error!("Error during enforce_all_trial_limits: {:?}", e);
            }
        }

        Ok(())
    }

    async fn restore_xray_trial_conns(&self) -> Result<()> {
        let conns_map = {
            let mem = self.sync.memory.read().await;
            mem.connections
                .iter()
                .filter(|(_, conn)| {
                    conn.get_trial() && !matches!(conn.get_proto().proto(), Tag::Wireguard)
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks = conns_map.into_iter().filter_map(|(conn_id, conn)| {
            let sync = self.sync.clone();
            let publisher = self.publisher.clone();

            let tag = conn.get_proto().proto();
            let wg = conn.get_wireguard();
            let password = conn.get_password();

            let reset_msg = Message {
                action: Action::ResetStat,
                conn_id,
                password: password.clone(),
                tag: tag,
                wg: wg.cloned(),
            };

            let restore_msg = Message {
                action: Action::Create,
                conn_id,
                password: password,
                tag: tag,
                wg: wg.cloned(),
            };

            Some(async move {
                match SyncOp::activate_trial_conn(&sync, &conn_id).await {
                    Ok(OperationStatus::Updated(_id)) => {
                        publisher.send(&conn.get_env(), restore_msg).await?;
                        publisher.send(&conn.get_env(), reset_msg).await?;
                        log::info!("Trial connection {} was restored", conn_id);
                        Ok(())
                    }
                    Ok(OperationStatus::UpdatedStat(_id)) => {
                        log::info!("Trial connection stat {} was restored", conn_id);
                        Ok(())
                    }
                    Ok(_) => Err(PonyError::Custom("Op isn't supported".into())),
                    Err(_) => Err(PonyError::Custom(
                        "SyncOp::activate_trial_conn failed".into(),
                    )),
                }
            })
        });

        let results = join_all(tasks).await;

        for result in results {
            if let Err(e) = result {
                log::error!("Error during restore_trial_conns: {:?}", e);
            }
        }

        Ok(())
    }
}
