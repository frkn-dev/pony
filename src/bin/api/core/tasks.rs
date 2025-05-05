use async_trait::async_trait;
use chrono::Duration;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Utc;
use futures::future::join_all;

use pony::clickhouse::query::Queries;
use pony::state::Conn;
use pony::state::ConnRow;
use pony::state::ConnStat;
use pony::state::ConnStatus;
use pony::state::ConnStorageApi;
use pony::state::Node;
use pony::state::NodeStatus;
use pony::state::NodeStorage;
use pony::state::SyncOp;
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
    async fn check_limit_and_expire_conns(&self) -> Result<()>;
    async fn restore_trial_conns(&self) -> Result<()>;
}

#[async_trait]
impl<T, C> Tasks for Api<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static + From<Conn>,
{
    async fn add_user(&self, db_user: UserRow) -> Result<()> {
        let user = User {
            username: db_user.username,
            created_at: db_user.created_at,
            modified_at: db_user.modified_at,
        };

        let user_id = db_user.user_id;

        log::debug!("--> {:?} Add user", user);

        let mut state = self.state.memory.lock().await;

        match state.users.try_add(user_id, user) {
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
        );

        log::debug!("--> {:?} Add connection", conn);

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
                let status = match ch.fetch_node_heartbeat::<f64>(&env, &uuid, &hostname).await {
                    Some(hb) => {
                        let ts = Utc.timestamp_opt(hb.latest, 0);
                        match ts {
                            chrono::LocalResult::Single(dt) if now - dt <= timeout => {
                                NodeStatus::Online
                            }
                            _ => NodeStatus::Offline,
                        }
                    }
                    None => NodeStatus::Offline,
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

    async fn check_limit_and_expire_conns(&self) -> Result<()> {
        let conns_map = {
            let state = self.state.memory.lock().await;
            state
                .connections
                .iter()
                .filter(|(_, conn)| conn.get_trial() && conn.get_status() == ConnStatus::Active)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks = conns_map.into_iter().map(|(conn_id, conn)| {
            let state = self.state.clone();
            let publisher = self.publisher.clone();

            let msg = Message {
                action: Action::Delete,
                conn_id: conn_id,
                password: None,
            };

            async move {
                let msg = msg.clone();
                if let Ok(status) = SyncOp::check_limit_and_expire_conn(&state, &conn_id).await {
                    if status == ConnStatus::Expired {
                        let _ = publisher.send(&conn.get_env(), msg).await;
                    }
                    Ok(())
                } else {
                    Err(PonyError::Custom(
                        "SyncOp::check_limit_and_expire_conn failed".to_string(),
                    ))
                }
            }
        });

        let results = join_all(tasks).await;

        for result in results {
            if let Err(e) = result {
                log::error!("Error during check_limit_and_expire_conns: {:?}", e);
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
                .filter(|(_, conn)| conn.get_trial() && conn.get_status() == ConnStatus::Expired)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let tasks = conns_map.into_iter().map(|(conn_id, conn)| {
            let state = self.state.clone();
            let publisher = self.publisher.clone();

            let msg = Message {
                action: Action::Delete,
                conn_id: conn_id,
                password: None,
            };

            async move {
                if let Ok(_) = SyncOp::activate_trial_conn(&state, &conn_id).await {
                    let _ = publisher.send(&conn.get_env(), msg).await;
                    Ok(())
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
                log::error!("Error during check_limit_and_expire_conns: {:?}", e);
            }
        }

        Ok(())
    }
}
