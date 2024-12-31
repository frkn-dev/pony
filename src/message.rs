use log::error;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::actions;

use crate::xray_op::client;
use crate::{state::State, user::User};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Message {
    pub user_id: Uuid,
    pub action: Action,
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_message = serde_json::to_string(self).expect("Failed to serialize message");

        write!(f, "{}", json_message)
    }
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    state: Arc<Mutex<State>>,
    config_daily_limit_mb: i64,
    debug: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match message.action {
        Action::Create => {
            let user_id = message.user_id;

            let daily_limit_mb = message.limit.unwrap_or(config_daily_limit_mb);
            let trial = message.trial.unwrap_or(true);

            let user = User::new(trial, daily_limit_mb, message.password.clone());

            match actions::create_users(
                message.user_id.clone(),
                message.password.clone(),
                clients,
                state.clone(),
            )
            .await
            {
                Ok(_) => {
                    let mut state_guard = state.lock().await;

                    state_guard
                        .add_user(user_id.clone(), user.clone(), debug)
                        .await
                        .map_err(|err| {
                            error!("Failed to add user {}: {:?}", message.user_id, err);
                            format!("Failed to add user {}", message.user_id).into()
                        })
                }
                Err(err) => {
                    error!("Failed to create user {}: {:?}", message.user_id, err);
                    Err(format!("Failed to create user {}", message.user_id).into())
                }
            }
        }
        Action::Delete => {
            if let Err(e) =
                actions::remove_users(message.user_id, state.clone(), clients.clone()).await
            {
                return Err(format!("Couldn't remove users from Xray: {}", e).into());
            } else {
                let mut state = state.lock().await;
                let _ = state.expire_user(message.user_id).await;
            }

            Ok(())
        }
        Action::Update => {
            let mut state_lock = state.lock().await;

            if let Some(user) = state_lock.get_user(message.user_id).await {
                if let Some(trial) = message.trial {
                    if trial != user.trial {
                        if let Err(e) = actions::create_users(
                            message.user_id,
                            message.password.clone(),
                            clients.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            return Err(format!(
                                "Couldn’t update trial for user {}: {}",
                                message.user_id, e
                            )
                            .into());
                        }

                        if let Err(e) = state_lock.update_user_trial(message.user_id, trial).await {
                            return Err(format!(
                                "Failed to update trial for user {}: {}",
                                message.user_id, e
                            )
                            .into());
                        }
                    }
                }

                if let Some(limit) = message.limit {
                    if let Err(e) = state_lock.update_user_limit(message.user_id, limit).await {
                        return Err(format!(
                            "Failed to update limit for user {}: {}",
                            message.user_id, e
                        )
                        .into());
                    }
                }

                Ok(())
            } else {
                Err(format!("User {} not found", message.user_id).into())
            }
        }
    }
}
