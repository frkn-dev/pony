use log::debug;

use tokio::sync::Mutex;

use std::error::Error;
use std::sync::Arc;

use crate::state::{node::Node, state::State};

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
