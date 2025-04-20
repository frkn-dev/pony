use std::error::Error;
use std::sync::Arc;

use chrono::Duration;
use chrono::LocalResult;
use chrono::TimeZone;
use chrono::Utc;
use clickhouse::Client as ChClient;
use futures::future::join_all;
use log::debug;
use log::error;
use log::warn;
use tokio::sync::Mutex;

use crate::state::state::NodeStorage;
use crate::state::state::UserStorage;
use crate::DbContext;
use crate::{
    clickhouse::query::fetch_heartbeat_value,
    postgres::user::UserRow,
    state::{node::Node, node::NodeStatus, state::State, user::User},
};

pub async fn node_healthcheck<T>(
    state: Arc<Mutex<State<T>>>,
    ch_client: Arc<Mutex<ChClient>>,
    db: DbContext,
    timeout: i16,
) -> Result<(), Box<dyn Error + Send + Sync>>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    let state = state.lock().await;

    debug!("Running healthcheck...");

    let nodes = match state.nodes.get_all_nodes() {
        Some(nodes) => nodes,
        None => {
            debug!("No nodes found for health check");
            return Ok(());
        }
    };

    let timeout_duration = Duration::seconds(timeout as i64);

    let tasks = nodes.into_iter().map(|mut node| {
        let client = ch_client.clone();
        let db = db.clone();

        async move {
            let result = fetch_heartbeat_value(client, &node.env, node.uuid).await;
            match result {
                Some(value) => {
                    debug!("Heartbeat value for node {:?}: {:?}", node.uuid, value);

                    let now = Utc::now();
                    let latest_datetime = Utc.timestamp_opt(value.latest, 0);
                    match latest_datetime {
                        LocalResult::Single(latest_datetime) => {
                            let time_diff = now - latest_datetime;

                            debug!("Time diff {}", time_diff);

                            match node.status {
                                NodeStatus::Online => {
                                    debug!("Time {}", time_diff > timeout_duration);
                                    if time_diff > timeout_duration {
                                        let _ = node.update_status(NodeStatus::Offline);
                                        let _ = db
                                            .node()
                                            .update_node_status(node.uuid, NodeStatus::Offline)
                                            .await;
                                    }
                                }
                                NodeStatus::Offline => {
                                    debug!("Time {}", time_diff <= timeout_duration);

                                    if time_diff <= timeout_duration {
                                        let _ = node.update_status(NodeStatus::Online);
                                        let _ = db
                                            .node()
                                            .update_node_status(node.uuid, NodeStatus::Online)
                                            .await;
                                    }
                                }
                                NodeStatus::Unknown => {
                                    warn!(
                                        "Node Status for {} {} is UNKNOWN ",
                                        node.uuid, node.hostname
                                    )
                                }
                            }
                            Ok(())
                        }
                        LocalResult::None => {
                            error!("Invalid timestamp for node {:?}", node.uuid);
                            Err("Invalid timestamp")
                        }
                        LocalResult::Ambiguous(.., _) => {
                            error!("Ambiguous timestamp for node {:?}", node.uuid);
                            Err("Ambiguous timestamp")
                        }
                    }
                }
                None => {
                    error!("Failed to fetch heartbeat for node {:?}", node.uuid);
                    let _ = node.update_status(NodeStatus::Offline);
                    let _ = db
                        .node()
                        .update_node_status(node.uuid, NodeStatus::Offline)
                        .await;
                    Err("No heartbeat value found")
                }
            }
        }
    });

    let results: Vec<_> = join_all(tasks).await;

    for result in results {
        match result {
            Ok(_) => debug!("Node health check succeeded"),
            Err(e) => error!("Node health check failed: {:?}", e),
        }
    }

    Ok(())
}

pub async fn init_state<T>(state: Arc<Mutex<State<T>>>, db_node: Node) -> Result<(), Box<dyn Error>>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let _ = {
        let mut user_state = state.lock().await;
        match user_state.nodes.add_node(db_node.clone()) {
            Ok(user) => {
                debug!("Node added to State {:?}", user);
                return Ok(());
            }
            Err(e) => {
                return Err(format!(
                    "Create: Failed to add user {} to state: {}",
                    db_node.uuid, e
                )
                .into());
            }
        }
    };
}

pub async fn sync_users<T>(
    state: Arc<Mutex<State<T>>>,
    db_user: UserRow,
) -> Result<(), Box<dyn Error>>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    let user = User::new(
        db_user.trial,
        db_user.limit,
        db_user.cluster,
        Some(db_user.password.clone()),
    );

    let _ = {
        let mut state = state.lock().await;
        match state.add_or_update_user(db_user.user_id, user.clone()) {
            Ok(_) => {}
            Err(e) => {
                return Err(format!(
                    "Create: Failed to add user {} to state: {}",
                    db_user.user_id, e
                )
                .into());
            }
        }
    };

    Ok(())
}
