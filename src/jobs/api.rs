use crate::state::user::User;
use log::debug;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    postgres::user::UserRow,
    state::{node::Node, state::State},
};

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
