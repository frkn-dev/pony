use log::debug;
use serde::Deserialize;
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::actions;

use crate::xray_op::client;
use crate::{state::State, user::User};

#[derive(Deserialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "init")]
    Init,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Message {
    pub user_id: Uuid,
    pub action: Action,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    state: Arc<Mutex<State>>,
    config_daily_limit_mb: i64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match message.action {
        Action::Create => {
            let state_lock = state.lock().await;

            let mut user_state = state_lock.clone();

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
                Ok(_) => user_state
                    .add_user(user_id.clone(), user.clone())
                    .await
                    .map_err(|_| format!("Failed to add user {}", message.user_id).into()),
                Err(_) => Err(format!("Failed to create user {}", message.user_id).into()),
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
            let user = state_lock.get_user(message.user_id).await;

            match (user, message.trial) {
                (Some(user), Some(trial)) if trial != user.trial => {
                    if let Err(e) = actions::create_users(
                        message.user_id,
                        message.password,
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
                    } else {
                        let mut state = state.lock().await;
                        let _ = state.update_user_trial(message.user_id, trial).await;
                    }
                }
                _ => {}
            }

            if let Some(limit) = message.limit {
                let _ = state_lock.update_user_limit(message.user_id, limit).await;
            }
            Ok(())
        }
        Action::Init => {
            debug!("This action needs to initiate TCP handshake and recieve next messages");
            Ok(())
        }
    }
}
