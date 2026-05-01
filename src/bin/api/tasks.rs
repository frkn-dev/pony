use chrono::Utc;
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

use tracing::{debug, error, info, warn};

use fcore::{
    measure_time, Connection, ConnectionBaseOperations, ConnectionStorageApiOperations, Env, Node,
    Result, Status, Subscription, SubscriptionOperations,
};

use super::{
    postgres::pg::Tasks as MemoryCacheTasks,
    service::{Cache, Service},
    sync::tasks::SyncOp,
};

#[async_trait::async_trait]
pub trait Tasks {
    async fn get_state_from_db(&self) -> Result<()>;
    async fn periodic_db_sync(&self, interval_sec: u64);
    async fn cleanup_expired_connections(&self, interval_sec: u64);
    async fn cleanup_expired_subscriptions(&self, interval_sec: u64);
    async fn restore_subscriptions(&self, interval_sec: u64);
}

#[async_trait::async_trait]
impl Tasks for Service<HashMap<Env, Vec<Node>>, Connection, Subscription> {
    async fn cleanup_expired_connections(&self, interval_sec: u64) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_sec));

        loop {
            interval.tick().await;

            debug!("Run cleanup conections task");

            let now = Utc::now();
            let expired_conns: Vec<(uuid::Uuid, Connection)> = {
                let memory = self.sync.memory.read().await;
                memory
                    .connections
                    .iter()
                    .filter_map(|(id, conn)| {
                        if let Some(expires_at) = conn.get_expires_at() {
                            if expires_at <= now && !conn.get_deleted() {
                                Some((*id, conn.clone()))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            for (conn_id, conn) in expired_conns {
                match SyncOp::delete_connection(&self.sync, &conn_id, &conn).await {
                    Ok(Status::Ok(_)) => {
                        info!("Expired connection {} deleted", conn_id);
                    }
                    Ok(status) => {
                        warn!("Connection {} could not be deleted: {:?}", conn_id, status);
                    }
                    Err(e) => {
                        error!("Failed to delete expired connection {}: {:?}", conn_id, e);
                    }
                }
            }
        }
    }

    async fn cleanup_expired_subscriptions(&self, interval_sec: u64) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_sec));

        loop {
            interval.tick().await;
            debug!("Run cleanup subscriptions task");

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
                                .map(|(id, c)| (*id, c.clone()))
                                .collect()
                        })
                        .unwrap_or_default()
                };

                for (conn_id, conn) in conns_to_delete {
                    match SyncOp::delete_connection(&self.sync, &conn_id, &conn).await {
                        Ok(Status::Ok(_)) => {
                            info!("Expired connection {} deleted", conn_id);
                        }
                        Ok(status) => {
                            warn!(
                                "!!! Connection {} could not be deleted: {:?}",
                                conn_id, status
                            );
                        }
                        Err(e) => {
                            error!("Failed to delete expired connection {}: {:?}", conn_id, e);
                        }
                    }
                }
            }
        }
    }

    async fn restore_subscriptions(&self, interval_sec: u64) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_sec));

        loop {
            interval.tick().await;
            debug!("Run restore subscriptions task");

            let active_subs: Vec<uuid::Uuid> = {
                let mem = self.sync.memory.read().await;
                mem.subscriptions
                    .iter()
                    .filter_map(|(id, sub)| if sub.is_active() { Some(*id) } else { None })
                    .collect()
            };

            for sub_id in active_subs {
                match SyncOp::restore_connections_by_subscription(&self.sync, &sub_id).await {
                    Ok(restored) => {
                        if !restored.is_empty() {
                            info!(
                                "Restored {} connections for subscription {}",
                                restored.len(),
                                sub_id
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to restore expired connection {}: {:?}", sub_id, e);
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

            if let Err(e) = measure_time(self.get_state_from_db(), "Periodic DB Sync").await {
                error!("Periodic DB sync failed: {:?}", e);
            } else {
                info!("Periodic DB sync completed successfully");
            }
        }
    }

    async fn get_state_from_db(&self) -> Result<()> {
        let db = self.sync.db.clone();
        let mut mem = self.sync.memory.write().await;

        let mut tmp_mem: Cache<HashMap<Env, Vec<Node>>, Connection, Subscription> = Cache::new();

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
}
