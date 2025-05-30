use async_trait::async_trait;
use chrono::Duration;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;
use std::collections::HashMap;

use pony::clickhouse::query::Queries;
use pony::state::Conn;
use pony::state::ConnRow;
use pony::state::ConnStat;
use pony::state::ConnStatus;
use pony::state::ConnStorageApi;
use pony::state::ConnStorageOpStatus;
use pony::state::Node;
use pony::state::NodeStatus;
use pony::state::NodeStorage;
use pony::state::SyncOp;
use pony::state::SyncTask;
use pony::state::User;
use pony::state::UserRow;
use pony::state::UserStorage;
use pony::state::{ConnApiOp, ConnBaseOp};
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::PonyError;
use pony::Result;

use crate::Api;

#[async_trait]
pub trait Tasks {
    async fn add_node(&self, db_node: Node) -> Result<()>;
    async fn add_conn(&self, db_conn: ConnRow) -> Result<()>;
    async fn add_user(&self, db_user: UserRow) -> Result<()>;
    async fn node_healthcheck(&self) -> Result<()>;
    async fn collect_conn_stat(&self) -> Result<()>;
    async fn enforce_all_trial_limits(&self) -> Result<()>;
    async fn restore_trial_conns(&self) -> Result<()>;
}

#[async_trait]
impl<T, C> Tasks for Api<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static + From<Conn>,
    Vec<(uuid::Uuid, pony::state::Conn)>: FromIterator<(uuid::Uuid, C)>,
{
    async fn add_user(&self, db_user: UserRow) -> Result<()> {
        let tg_id = if let Some(tg_id) = db_user.telegram_id {
            Some(tg_id as u64)
        } else {
            None
        };
        let user = User {
            username: db_user.username,
            telegram_id: tg_id,
            env: db_user.env,
            limit: db_user.limit,
            password: db_user.password,
            created_at: db_user.created_at,
            modified_at: db_user.modified_at,
            is_deleted: db_user.is_deleted,
        };

        let user_id = db_user.user_id;
        let mut state = self.state.memory.lock().await;

        match state.users.try_add(&user_id, user) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Create: Failed to add user {} to state: {}",
                db_user.user_id, e
            )
            .into()),
        }
    }

    async fn add_conn(&self, db_conn: ConnRow) -> Result<()> {
        let conn = Conn::new(
            db_conn.trial,
            db_conn.limit,
            &db_conn.env,
            db_conn.status,
            Some(db_conn.password.clone()),
            db_conn.user_id,
            db_conn.stat,
            db_conn.proto,
        );

        let mut state = self.state.memory.lock().await;
        match state
            .connections
            .add_or_update(&db_conn.conn_id.clone(), conn.clone().into())
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Create: Failed to add connection {} to state: {}",
                db_conn.conn_id, e
            )
            .into()),
        }
    }
    async fn add_node(&self, db_node: Node) -> Result<()> {
        let mut state = self.state.memory.lock().await;
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
        let mem = self.state.memory.lock().await;
        let nodes = match mem.nodes.all() {
            Some(n) => n.clone(),
            None => return Ok(()),
        };
        drop(mem);

        let timeout = Duration::seconds(self.settings.api.node_health_check_timeout as i64);
        let now = Utc::now();

        let tasks = nodes.into_iter().map(|node| {
            let ch = self.ch.clone();
            let state = self.state.clone();
            let uuid = node.uuid;
            let env = node.env.clone();
            let hostname = node.hostname;

            async move {
                let status =
                    match ch.fetch_node_heartbeat::<f64>(&env, &uuid, &hostname).await {
                        Some(hb) => {
                            let ts = Utc.timestamp_opt(hb.latest, 0);
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
                                        hb.latest
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

                SyncOp::update_node_status(&state, &uuid, &env, status).await?;
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
            let state = self.state.memory.lock().await;
            state
                .connections
                .iter()
                .filter(|(_, conn)| conn.get_status() == ConnStatus::Active)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let today = Utc::now().date_naive();
        let start_of_day =
            Utc.from_utc_datetime(&today.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));

        let tasks = conns_map.into_iter().map(|(conn_id, _)| {
            let ch = self.ch.clone();
            let state = self.state.clone();

            async move {
                if let Some(metrics) = ch.fetch_conn_stats::<i64>(conn_id, start_of_day).await {
                    log::debug!("metrics {:?}", metrics);
                    let stat = ConnStat::from_metrics(metrics);
                    log::debug!("Stat to update - {}", stat);
                    SyncOp::update_conn_stat(&state, &conn_id, stat).await
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

    async fn enforce_all_trial_limits(&self) -> Result<()> {
        let conns: Vec<(uuid::Uuid, Conn)> = {
            let state = self.state.memory.lock().await;
            state
                .connections
                .iter()
                .filter_map(|(id, conn)| {
                    if conn.get_trial() && conn.get_status() == ConnStatus::Active {
                        Some((*id, conn.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut grouped: HashMap<Option<uuid::Uuid>, Vec<(uuid::Uuid, Conn)>> = HashMap::new();
        for (conn_id, conn) in conns {
            grouped
                .entry(conn.get_user_id())
                .or_default()
                .push((conn_id, conn));
        }

        let state = self.state.clone();
        let publisher = self.publisher.clone();

        let tasks = grouped.into_iter().map(move |(_user_id, conns)| {
            let state = state.clone();
            let publisher = publisher.clone();

            async move {
                let total_downlink: i64 = conns.iter().map(|(_, c)| c.get_downlink()).sum();

                let limit = conns.iter().map(|(_, c)| c.get_limit()).max().unwrap_or(0);

                if (total_downlink as f64 / 1_048_576.0) >= limit as f64 {
                    for (conn_id, conn) in conns {
                        {
                            let mut mem = state.memory.lock().await;
                            if let Some(conn) = mem.connections.get_mut(&conn_id) {
                                conn.set_status(ConnStatus::Expired);
                                conn.set_modified_at();
                            }
                        }

                        let _ = state
                            .sync_tx
                            .send(SyncTask::UpdateConnStatus {
                                conn_id,
                                status: ConnStatus::Expired,
                            })
                            .await;

                        let msg = Message {
                            action: Action::Delete,
                            conn_id,
                            password: conn.get_password(),
                            tag: conn.get_proto(),
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

    async fn restore_trial_conns(&self) -> Result<()> {
        let conns_map = {
            let state = self.state.memory.lock().await;
            state
                .connections
                .iter()
                .filter(|(_, conn)| conn.get_trial())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks = conns_map.into_iter().map(|(conn_id, conn)| {
            let state = self.state.clone();
            let publisher = self.publisher.clone();

            let reset_msg = Message {
                action: Action::ResetStat,
                conn_id: conn_id,
                password: None,
                tag: conn.get_proto(),
            };

            let restore_msg = Message {
                action: Action::Create,
                conn_id: conn_id,
                password: None,
                tag: conn.get_proto(),
            };

            async move {
                if let Ok(status) = SyncOp::activate_trial_conn(&state, &conn_id).await {
                    match status {
                        ConnStorageOpStatus::Updated => {
                            let _ = publisher.send(&conn.get_env(), restore_msg).await?;
                            let _ = publisher.send(&conn.get_env(), reset_msg).await?;
                            log::info!("Trial connection {} was restored", conn_id);

                            Ok(())
                        }
                        ConnStorageOpStatus::UpdatedStat => {
                            log::info!("Trial connection stat {} was restored", conn_id);

                            Ok(())
                        }
                        _ => Err(PonyError::Custom("Op isnt' supported".to_string())),
                    }
                } else {
                    Err(PonyError::Custom(
                        "SyncOp::activate_trial_conn failed".to_string(),
                    ))
                }
            }
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
