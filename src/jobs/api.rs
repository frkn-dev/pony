use chrono::Duration;
use chrono::LocalResult;
use chrono::TimeZone;
use chrono::Utc;
use clickhouse::Client;
use futures::future::join_all;
use log::debug;
use log::error;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    clickhouse::query::fetch_heartbeat_value,
    postgres::user::UserRow,
    state::{node::Node, node::NodeStatus, state::State, user::User},
};

pub async fn node_healthcheck(
    state: Arc<Mutex<State>>,
    ch_client: Arc<Mutex<Client>>,
    timeout: i16,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let state = state.lock().await;

    let nodes = match state.get_all_nodes() {
        Some(nodes) => nodes,
        None => {
            debug!("No nodes found for health check");
            return Ok(());
        }
    };

    let timeout_duration = Duration::seconds(timeout as i64);

    let tasks = nodes.into_iter().map(|mut node| {
        let client = ch_client.clone();
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

                            match node.status {
                                NodeStatus::Online => {
                                    if time_diff > timeout_duration {
                                        let _ = node.update_status(NodeStatus::Offline);
                                    }
                                }
                                NodeStatus::Offline => {
                                    if time_diff <= timeout_duration {
                                        let _ = node.update_status(NodeStatus::Online);
                                    }
                                }
                                NodeStatus::Unknown => {}
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

pub async fn init_state(state: Arc<Mutex<State>>, db_node: Node) -> Result<(), Box<dyn Error>> {
    let _ = {
        let mut user_state = state.lock().await;
        match user_state.add_node(db_node.clone()).await {
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

pub async fn sync_users(
    state: Arc<Mutex<State>>,
    db_user: UserRow,
    limit: i64,
) -> Result<(), Box<dyn Error>> {
    let user = User::new(
        db_user.trial,
        limit,
        db_user.cluster,
        Some(db_user.password.clone()),
    );

    let _ = {
        let mut user_state = state.lock().await;
        match user_state.add_user(db_user.user_id, user.clone()).await {
            Ok(user) => {
                debug!("User added to State {:?}", user);
            }
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
