use crate::xray_op::users::UserState;
use log::debug;
use std::sync::Arc;
use tokio::sync::Mutex;

use chrono::{Duration, Utc};

pub async fn restore_trial_users(state: Arc<Mutex<UserState>>) {
    let trial_users = state.lock().await.get_all_trial_users();
    let now = Utc::now();

    for user in trial_users {
        let state = state.clone();
        let user_id = user.user_id.clone();

        let restore_user_task = async move {
            let mut state = state.lock().await;

            let user_to_restore = if let Some(modified_at) = user.modified_at {
                debug!("Check user for modified_at {:?}", user);
                now.signed_duration_since(modified_at) >= Duration::hours(24)
            } else {
                debug!("Check user for created_at {:?}", user);
                now.signed_duration_since(user.created_at) >= Duration::hours(24)
            };

            if user_to_restore {
                let _ = state.restore_user(user_id).await;
            }
        };

        tokio::spawn(restore_user_task);
    }
}
