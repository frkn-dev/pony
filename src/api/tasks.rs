use std::error::Error;

use async_trait::async_trait;
use chrono::Duration;
use chrono::LocalResult;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use log::debug;
use log::error;
use log::warn;

use crate::clickhouse::query::Queries;
use crate::Action;
use crate::Api;
use crate::Message;
use crate::Node;
use crate::NodeStatus;
use crate::NodeStorage;
use crate::User;
use crate::UserRow;
use crate::UserStorage;

#[async_trait]
pub trait Tasks {
    async fn node_healthcheck(&self, timeout: i16) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn check_user_uplink_limits(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn add_node(&self, db_node: Node) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn add_user(&self, db_user: UserRow) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
impl<T: NodeStorage + std::marker::Send + std::marker::Sync + std::clone::Clone> Tasks for Api<T> {
    async fn add_user(&self, db_user: UserRow) -> Result<(), Box<dyn Error + Send + Sync>> {
        let user = User::new(
            db_user.trial,
            db_user.limit,
            db_user.cluster.clone(),
            Some(db_user.password.clone()),
        );

        let mut state = self.state.lock().await;
        match state.add_or_update_user(db_user.user_id.clone(), user.clone()) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Create: Failed to add user {} to state: {}",
                db_user.user_id, e
            )
            .into()),
        }
    }
    async fn add_node(&self, db_node: Node) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut user_state = self.state.lock().await;
        match user_state.nodes.add_node(db_node.clone()) {
            Ok(_) => {
                debug!("Node added to State: {}", db_node.uuid);
                Ok(())
            }
            Err(e) => Err(format!(
                "Create: Failed to add node {} to state: {}",
                db_node.uuid, e
            )
            .into()),
        }
    }
    async fn node_healthcheck(&self, timeout: i16) -> Result<(), Box<dyn Error + Send + Sync>> {
        let state_lock = self.state.lock().await;
        let nodes = match state_lock.nodes.get_all_nodes() {
            Some(n) => n.clone(),
            None => return Ok(()),
        };
        drop(state_lock);

        let timeout_duration = Duration::seconds(timeout as i64);

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
                            if let Some(node_mut) = state.nodes.get_mut_node(&env, uuid) {
                                match node_mut.status {
                                    NodeStatus::Online if time_diff > timeout_duration => {
                                        node_mut.update_status(NodeStatus::Offline)?;
                                        let _ = db
                                            .node()
                                            .update_node_status(uuid, &env, NodeStatus::Offline)
                                            .await;
                                    }
                                    NodeStatus::Offline if time_diff <= timeout_duration => {
                                        node_mut.update_status(NodeStatus::Online)?;
                                        let _ = db
                                            .node()
                                            .update_node_status(uuid, &env, NodeStatus::Online)
                                            .await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    None => {
                        let mut state = state.lock().await;
                        if let Some(node_mut) = state.nodes.get_mut_node(&env, uuid) {
                            node_mut.update_status(NodeStatus::Offline)?;
                        }
                        let _ = db
                            .node()
                            .update_node_status(uuid, &env, NodeStatus::Offline)
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

    async fn check_user_uplink_limits(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let users_map = {
            let state = self.state.lock().await;
            state
                .users
                .iter()
                .filter(|(_, user)| user.trial)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks = users_map.into_iter().map(|(user_id, user)| {
            let ch = self.ch.clone();
            let publisher = self.publisher.clone();
            let env = user.env.clone();
            let modified_at = user.modified_at;

            async move {
                let result = ch
                    .fetch_user_uplink_traffic::<f64>(&env, user_id, modified_at)
                    .await;

                if let Some(metric) = result {
                    let uplink_mb = metric.value / 1_048_576.0;

                    if uplink_mb > user.limit as f64 {
                        warn!(
                            "🚨 User {} in env {} exceeds limit: uplink = {:.2} MB / {} MB",
                            user_id, env, uplink_mb, user.limit
                        );

                        let msg = Message {
                            user_id: user_id.clone(),
                            action: Action::Delete,
                            env: env.clone(),
                            trial: user.trial,
                            limit: user.limit,
                            password: None,
                        };

                        if let Err(e) = publisher.send(&env, msg).await {
                            error!("Failed to send DELETE for {}: {}", user_id, e);
                        }
                    } else {
                        debug!(
                            "✅ User {} uplink OK: {:.2} MB / {} MB",
                            user_id, uplink_mb, user.limit
                        );
                    }
                } else {
                    debug!("No uplink data for user {}", user_id);
                }

                Ok::<(), Box<dyn Error + Send + Sync>>(())
            }
        });

        let results = join_all(tasks).await;

        for res in results {
            if let Err(e) = res {
                error!("Error during user uplink check: {:?}", e);
            }
        }

        Ok(())
    }
}
