use async_trait::async_trait;
use chrono::Duration;
use chrono::LocalResult;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use std::error::Error;

use pony::clickhouse::query::Queries;
use pony::postgres::connection::ConnRow;
use pony::state::connection::Conn;
use pony::state::connection::ConnStatus;
use pony::state::node::Node;
use pony::state::node::NodeStatus;
use pony::state::state::ConnStorage;
use pony::state::state::NodeStorage;
use pony::zmq::message::Action;
use pony::zmq::message::Message;

use super::Api;

#[async_trait]
pub trait Tasks {
    async fn node_healthcheck(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn check_conn_uplink_limits(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn reactivate_trial_conns(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn add_node(&self, db_node: Node) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn add_conn(&self, db_conn: ConnRow) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
impl<T: NodeStorage + Send + Sync + Clone> Tasks for Api<T> {
    async fn add_conn(&self, db_conn: ConnRow) -> Result<(), Box<dyn Error + Send + Sync>> {
        let conn = Conn::new(
            db_conn.trial,
            db_conn.limit,
            db_conn.env.clone(),
            Some(db_conn.password.clone()),
        );

        let mut state = self.state.lock().await;
        match state
            .connections
            .add_or_update(&db_conn.conn_id.clone(), conn.clone())
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Create: Failed to add connection {} to state: {}",
                db_conn.conn_id, e
            )
            .into()),
        }
    }
    async fn add_node(&self, db_node: Node) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut state = self.state.lock().await;
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
    async fn node_healthcheck(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let state_lock = self.state.lock().await;
        let nodes = match state_lock.nodes.all() {
            Some(n) => n.clone(),
            None => return Ok(()),
        };
        drop(state_lock);

        let timeout_duration =
            Duration::seconds(self.settings.api.node_health_check_timeout as i64);

        let tasks = nodes.into_iter().map(|node| {
            let ch = self.ch.clone();
            let db = self.db.clone();
            let state = self.state.clone();
            let uuid = node.uuid;
            let env = node.env.clone();

            async move {
                let result = ch.fetch_node_heartbeat::<f64>(&env, uuid).await;
                match result {
                    Some(value) => {
                        let now = Utc::now();
                        let latest_datetime = Utc.timestamp_opt(value.latest, 0);
                        if let LocalResult::Single(latest_datetime) = latest_datetime {
                            let time_diff = now - latest_datetime;
                            let mut state = state.lock().await;
                            if let Some(node_mut) = state.nodes.get_mut(&env, &uuid) {
                                match node_mut.status {
                                    NodeStatus::Online if time_diff > timeout_duration => {
                                        node_mut.update_status(NodeStatus::Offline)?;
                                        let _ = db
                                            .node()
                                            .update_status(uuid, &env, NodeStatus::Offline)
                                            .await;
                                    }
                                    NodeStatus::Offline if time_diff <= timeout_duration => {
                                        node_mut.update_status(NodeStatus::Online)?;
                                        let _ = db
                                            .node()
                                            .update_status(uuid, &env, NodeStatus::Online)
                                            .await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    None => {
                        let mut state = state.lock().await;
                        if let Some(node_mut) = state.nodes.get_mut(&env, &uuid) {
                            node_mut.update_status(NodeStatus::Offline)?;
                        }
                        let _ = db
                            .node()
                            .update_status(uuid, &env, NodeStatus::Offline)
                            .await;
                    }
                }
                Ok::<(), Box<dyn Error + Send + Sync>>(())
            }
        });

        let results = join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Healthcheck task error: {:?}", e);
            }
        }

        Ok(())
    }

    async fn check_conn_uplink_limits(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let conns_map = {
            let state = self.state.lock().await;
            state
                .connections
                .iter()
                .filter(|(_, conn)| conn.trial && conn.status == ConnStatus::Active)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks =
            conns_map.into_iter().map(|(conn_id, conn)| {
                let ch = self.ch.clone();
                let publisher = self.publisher.clone();
                let state = self.state.clone();
                let env = conn.env.clone();
                let modified_at = conn.modified_at;

                async move {
                    let result = ch
                        .fetch_conn_uplink_traffic::<f64>(&env, conn_id, modified_at)
                        .await;

                    if let Some(metric) = result {
                        let uplink_mb = metric.value / 1_048_576.0;

                        if uplink_mb > conn.limit as f64 {
                            log::warn!(
                            "🚨 Connection {} in env {} exceeds limit: uplink = {:.2} MB / {} MB",
                            conn_id, env, uplink_mb, conn.limit
                        );

                            let msg = Message {
                                conn_id: conn_id.clone(),
                                action: Action::Delete,
                                env: env.clone(),
                                trial: conn.trial,
                                limit: conn.limit,
                                password: None,
                            };

                            if let Err(e) = publisher.send(&env, msg).await {
                                log::error!("Failed to send DELETE for {}: {}", conn_id, e);
                            }

                            {
                                let mut state = state.lock().await;
                                if let Some(conn) = state.connections.get_mut(&conn_id) {
                                    conn.status = ConnStatus::Expired;
                                    conn.update_modified_at();
                                    log::debug!("✅ Marked connection {} as Expired", conn_id);
                                }
                            }
                        } else {
                            log::debug!(
                                "✅ Connection {} uplink OK: {:.2} MB / {} MB",
                                conn_id,
                                uplink_mb,
                                conn.limit
                            );
                        }
                    } else {
                        log::debug!("No uplink data for connection {}", conn_id);
                    }

                    Ok::<(), Box<dyn Error + Send + Sync>>(())
                }
            });

        let results = join_all(tasks).await;

        for res in results {
            if let Err(e) = res {
                log::error!("Error during connection uplink check: {:?}", e);
            }
        }

        Ok(())
    }

    async fn reactivate_trial_conns(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let conns_map = {
            let state = self.state.lock().await;
            state
                .connections
                .iter()
                .filter(|(_, conn)| conn.trial && conn.status == ConnStatus::Expired)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks = conns_map.into_iter().map(|(conn_id, conn)| {
            let publisher = self.publisher.clone();
            let state = self.state.clone();
            let now = chrono::Utc::now();
            let modified_at = conn.modified_at;
            let env = conn.env.clone();

            async move {
                if now.signed_duration_since(modified_at) >= chrono::Duration::hours(24) {
                    {
                        let mut state = state.lock().await;
                        if let Some(conn_mut) = state.connections.get_mut(&conn_id) {
                            conn_mut.status = ConnStatus::Active;
                            conn_mut.update_modified_at();
                            log::debug!("✅ Re-activated connection {}", conn_id);
                        }
                    }

                    let msg = Message {
                        conn_id: conn_id.clone(),
                        action: Action::Create,
                        env: env.clone(),
                        trial: conn.trial,
                        limit: conn.limit,
                        password: Some(conn.password.clone().unwrap_or_default()),
                    };

                    if let Err(e) = publisher.send(&env, msg).await {
                        log::error!("Failed to send CREATE for {}: {}", conn_id, e);
                    }
                }

                Ok::<(), Box<dyn Error + Send + Sync>>(())
            }
        });

        let results = join_all(tasks).await;

        for res in results {
            if let Err(e) = res {
                log::error!("Error during connection reactivation: {:?}", e);
            }
        }

        Ok(())
    }
}
