use async_trait::async_trait;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use pony::http::requests::ConnUpdateRequest;
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::utils::measure_time;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStorageApiOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::PonyError;
use pony::Result;
use pony::Subscription;
use pony::SubscriptionOp;

use crate::core::clickhouse::query::Queries;
use crate::core::postgres::Tasks as MemoryCacheTasks;
use crate::core::sync::tasks::SyncOp;
use crate::Api;
use crate::ZmqPublisher;

#[async_trait]
pub trait Tasks {
    async fn get_state_from_db(&self) -> Result<()>;
    async fn periodic_db_sync(&self, interval_sec: u64);
    async fn node_healthcheck(&self) -> Result<()>;
    async fn collect_conn_stat(&self) -> Result<()>;
    async fn cleanup_expired_connections(&self, interval_sec: u64, publisher: ZmqPublisher);
    async fn cleanup_expired_subscriptions(&self, interval_sec: u64, publisher: ZmqPublisher);
    async fn restore_subscriptions(&self, interval_sec: u64, publisher: ZmqPublisher);
}

#[async_trait]
impl Tasks for Api<HashMap<String, Vec<Node>>, Connection, Subscription> {
    async fn cleanup_expired_connections(&self, interval_sec: u64, publisher: ZmqPublisher) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_sec));

        loop {
            interval.tick().await;

            log::debug!("Run cleanup conections task");

            let now = Utc::now();
            let expired_conns: Vec<uuid::Uuid> = {
                let memory = self.sync.memory.read().await;
                memory
                    .connections
                    .iter()
                    .filter_map(|(id, conn)| {
                        if let Some(expired_at) = conn.get_expired_at() {
                            if expired_at <= now && !conn.get_deleted() {
                                Some(*id)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            for conn_id in expired_conns {
                match SyncOp::delete_connection(&self.sync, &conn_id).await {
                    Ok(StorageOperationStatus::Ok(_)) => {
                        log::info!("Expired connection {} deleted", conn_id);

                        if let Some(conn) = self.sync.memory.read().await.connections.get(&conn_id)
                        {
                            let msg = conn.as_delete_message(&conn_id);
                            let bytes = match rkyv::to_bytes::<_, 1024>(&msg) {
                                Ok(b) => b,
                                Err(e) => {
                                    log::error!(
                                        "Serialization error for DELETE {}: {}",
                                        conn_id,
                                        e
                                    );
                                    continue;
                                }
                            };

                            let key = conn
                                .node_id
                                .map(|id| id.to_string())
                                .unwrap_or_else(|| conn.get_env());

                            let _ = publisher.send_binary(&key, bytes.as_ref()).await;
                        }
                    }
                    Ok(status) => {
                        log::warn!("Connection {} could not be deleted: {:?}", conn_id, status);
                    }
                    Err(e) => {
                        log::error!("Failed to delete expired connection {}: {:?}", conn_id, e);
                    }
                }
            }
        }
    }

    async fn cleanup_expired_subscriptions(&self, interval_sec: u64, publisher: ZmqPublisher) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_sec));

        loop {
            interval.tick().await;
            log::debug!("Run cleanup subscriptions task");

            let expired_subs: Vec<uuid::Uuid> = {
                let mem = self.sync.memory.read().await;
                mem.subscriptions
                    .iter()
                    .filter_map(|(id, sub)| if !sub.is_active() { Some(*id) } else { None })
                    .collect()
            };

            for sub_id in expired_subs {
                let conns_to_delete: Vec<(uuid::Uuid, Connection)> = {
                    let mem = self.sync.memory.read().await;
                    mem.connections
                        .get_by_subscription_id(&sub_id)
                        .map(|conns| {
                            conns
                                .iter()
                                .filter(|(_id, c)| !c.get_deleted())
                                .filter_map(|(id, c)| Some((*id, c.clone().into())))
                                .collect()
                        })
                        .unwrap_or_default()
                };

                for (conn_id, conn) in conns_to_delete {
                    let msg = conn.as_delete_message(&conn_id);
                    if let Ok(bytes) = rkyv::to_bytes::<_, 1024>(&msg) {
                        let key = conn
                            .node_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| conn.get_env());
                        let _ = publisher.send_binary(&key, bytes.as_ref()).await;
                    }

                    match SyncOp::delete_connection(&self.sync, &conn_id).await {
                        Ok(StorageOperationStatus::Ok(_)) => {
                            log::info!("Expired connection {} deleted", conn_id);
                        }
                        Ok(status) => {
                            log::warn!(
                                "!!! Connection {} could not be deleted: {:?}",
                                conn_id,
                                status
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to delete expired connection {}: {:?}", conn_id, e);
                        }
                    }
                }
            }
        }
    }

    async fn restore_subscriptions(&self, interval_sec: u64, publisher: ZmqPublisher) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_sec));

        loop {
            interval.tick().await;
            log::debug!("Run restore subscriptions task");

            let expired_subs: Vec<uuid::Uuid> = {
                let mem = self.sync.memory.read().await;
                mem.subscriptions
                    .iter()
                    .filter_map(|(id, sub)| if sub.is_active() { Some(*id) } else { None })
                    .collect()
            };

            for sub_id in expired_subs {
                let conns_to_restore: Vec<(uuid::Uuid, Connection)> = {
                    let mem = self.sync.memory.read().await;
                    mem.connections
                        .get_by_subscription_id(&sub_id)
                        .map(|conns| {
                            conns
                                .iter()
                                .filter(|(_id, c)| c.get_deleted())
                                .filter_map(|(id, c)| Some((*id, c.clone().into())))
                                .collect()
                        })
                        .unwrap_or_default()
                };

                for (conn_id, conn) in conns_to_restore {
                    let msg = conn.as_update_message(&conn_id);
                    if let Ok(bytes) = rkyv::to_bytes::<_, 1024>(&msg) {
                        let key = conn
                            .node_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| conn.get_env());
                        let _ = publisher.send_binary(&key, bytes.as_ref()).await;
                    }

                    let conn_upd = ConnUpdateRequest {
                        env: Some(conn.get_env()),
                        is_deleted: Some(false),
                        password: conn.get_password(),
                        days: None,
                    };

                    match SyncOp::update_conn(&self.sync, &conn_id, conn_upd).await {
                        Ok(StorageOperationStatus::Updated(_)) => {
                            log::info!("Expired connection {} restored", conn_id);
                        }
                        Ok(status) => {
                            log::warn!(
                                "Connection {} could not be restored: {:?}",
                                conn_id,
                                status
                            );
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to restore expired connection {}: {:?}",
                                conn_id,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    async fn periodic_db_sync(&self, interval_sec: u64) {
        let base = Duration::from_secs(interval_sec);

        loop {
            let jitter = rand::thread_rng().gen_range(0..=30);
            let interval_sec_with_jitter = base + Duration::from_secs(jitter);
            tokio::time::sleep(interval_sec_with_jitter).await;

            if let Err(e) = measure_time(
                self.get_state_from_db(),
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

    async fn get_state_from_db(&self) -> Result<()> {
        let db = self.sync.db.clone();
        let mut mem = self.sync.memory.write().await;

        let mut tmp_mem: MemoryCache<HashMap<String, Vec<Node>>, Connection, Subscription> =
            MemoryCache::new();

        let node_repo = db.node();
        let conn_repo = db.conn();
        let sub_repo = db.sub();

        let (nodes, conns, subscriptions) =
            tokio::try_join!(node_repo.all(), conn_repo.all(), sub_repo.all())?;

        for node in nodes {
            tmp_mem.add_node(node).await?;
        }
        for conn in conns {
            tmp_mem.add_conn(conn).await?;
        }
        for sub in subscriptions {
            tmp_mem.add_subscription(sub).await;
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
