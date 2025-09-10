use async_trait::async_trait;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use pony::ConnectionBaseOp;
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::utils::measure_time;
use pony::Connection;
use pony::ConnectionStat;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::PonyError;
use pony::Result;

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
                                        "Could not parse timestamp from heartbeat: {:?}",
                                        hb.timestamp
                                    );
                                    return Err(PonyError::Custom(format!(
                                        "Could not parse timestamp from heartbeat: {:?}",
                                        hb.timestamp
                                    )));
                                }
                            }
                        }
                        None => {
                            log::debug!(
                                "No heartbeat data found for node {} ({}) in env {}",
                                uuid,
                                hostname,
                                env
                            );
                            return Err(PonyError::Custom(format!(
                                "No heartbeat data found for node {} ({}) in env {}",
                                uuid, hostname, env
                            )));
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
                .filter(|(_, conn)| !conn.get_deleted())
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
}
