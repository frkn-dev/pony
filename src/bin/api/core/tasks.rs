use async_trait::async_trait;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::utils::measure_time;
use pony::Action;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::MemoryCache;
use pony::Message;
use pony::NodeStorageOp;
use pony::OperationStatus;
use pony::PonyError;
use pony::Result;
use pony::Tag;

use crate::core::clickhouse::query::Queries;
use crate::core::postgres::Tasks as MemoryCacheTasks;
use crate::core::sync::tasks::SyncOp;
use crate::Api;

#[async_trait]
pub trait Tasks {
    async fn init_state_from_db(&self) -> Result<()>;
    async fn periodic_db_sync(&self, interval_sec: u64);
    async fn node_healthcheck(&self) -> Result<()>;
    async fn collect_conn_stat(&self) -> Result<()>;
    async fn enforce_xray_trial_limits(&self) -> Result<()>;
    async fn restore_xray_trial_conns(&self) -> Result<()>;
}

#[async_trait]
impl Tasks for Api<HashMap<String, Vec<Node>>, Connection> {
    async fn periodic_db_sync(&self, interval_sec: u64) {
        let base = Duration::from_secs(interval_sec);

        loop {
            let jitter = rand::thread_rng().gen_range(0..=30);
            let interval_sec_with_jitter = base + Duration::from_secs(jitter);
            tokio::time::sleep(interval_sec_with_jitter).await;

            if let Err(e) = measure_time(
                self.init_state_from_db(),
                format!("Periodic DB Sync: interval + jitter {interval_sec}, {jitter}"),
            )
            .await
            {
                log::error!("Periodic DB sync failed: {:?}", e);
            } else {
                log::info!("Periodic DB sync completed successfully");
            }
        }
    }

    async fn init_state_from_db(&self) -> Result<()> {
        let db = self.sync.db.clone();
        let mut mem = self.sync.memory.write().await;

        let mut tmp_mem: MemoryCache<HashMap<String, Vec<Node>>, Connection> = MemoryCache::new();

        let node_repo = db.node();
        let conn_repo = db.conn();

        let (nodes, conns) = tokio::try_join!(node_repo.all(), conn_repo.all(),)?;

        for node in nodes {
            tmp_mem.add_node(node).await?;
        }
        for conn in conns {
            tmp_mem.add_conn(conn).await?;
        }

        *mem = tmp_mem;

        Ok(())
    }

    async fn node_healthcheck(&self) -> Result<()> {
        let mem = self.sync.memory.read().await;
        let nodes = match mem.nodes.all() {
            Some(n) => n.clone(),
            None => return Ok(()),
        };
        drop(mem);

        let timeout = chrono::Duration::seconds(self.settings.api.node_health_check_timeout as i64);
        let now = Utc::now();

        let tasks = nodes.into_iter().map(|node| {
            let ch = self.ch.clone();
            let sync = self.sync.clone();
            let uuid = node.uuid;
            let env = node.env.clone();
            let hostname = node.hostname;

            async move {
                let status =
                    match ch.fetch_node_heartbeat(&env, &uuid, &hostname).await {
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

                if let Err(e) = SyncOp::update_node_status(&sync, &uuid, &env, status).await {
                    log::error!("Failed to update_node_status {}  database: {}", uuid, e);
                    return Err(PonyError::Custom(e.to_string()));
                }
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
                if let Some(metrics) = ch.fetch_conn_stats(conn_id, start_of_day).await {
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
                        self.sync
                            .db
                            .conn()
                            .update_status(&conn_id, ConnectionStatus::Expired)
                            .await?;

                        let mut mem = sync.memory.write().await;
                        if let Some(conn_mut) = mem.connections.get_mut(&conn_id) {
                            conn_mut.set_status(ConnectionStatus::Expired);
                            conn_mut.set_modified_at();
                        }

                        let msg = Message {
                            action: Action::Delete,
                            conn_id: conn_id.into(),
                            password: conn.get_password(),
                            tag: conn.get_proto().proto().into(),
                            wg: None,
                        };

                        let bytes = match rkyv::to_bytes::<_, 1024>(&msg) {
                            Ok(b) => b,
                            Err(e) => {
                                log::error!(
                                    "Failed to serialize delete message for connection {}: {}",
                                    conn_id,
                                    e
                                );
                                continue;
                            }
                        };

                        let _ = publisher.send_binary(&conn.get_env(), bytes.as_ref()).await;

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
                conn_id: conn_id.into(),
                password: password.clone(),
                tag: tag,
                wg: wg.cloned(),
            };

            let restore_msg = Message {
                action: Action::Create,
                conn_id: conn_id.into(),
                password: password,
                tag: tag,
                wg: wg.cloned(),
            };

            Some(async move {
                match SyncOp::activate_trial_conn(&sync, &conn_id).await {
                    Ok(OperationStatus::Updated(_id)) => {
                        let restore_bytes =
                            rkyv::to_bytes::<_, 1024>(&restore_msg).map_err(|e| {
                                PonyError::SerializationError(format!(
                                    "rkyv serialization failed: {}",
                                    e
                                ))
                            })?;
                        let reset_bytes = rkyv::to_bytes::<_, 1024>(&reset_msg).map_err(|e| {
                            PonyError::SerializationError(format!(
                                "rkyv serialization failed: {}",
                                e
                            ))
                        })?;
                        publisher
                            .send_binary(&conn.get_env(), restore_bytes.as_ref())
                            .await?;
                        publisher
                            .send_binary(&conn.get_env(), reset_bytes.as_ref())
                            .await?;

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
