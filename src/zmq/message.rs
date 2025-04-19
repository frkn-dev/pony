use std::error::Error;
use std::fmt;
use std::sync::Arc;

use log::debug;
use log::error;
use log::info;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::state::state::NodeStorage;
use crate::state::state::State;
use crate::state::state::UserStorage;
use crate::state::user::User;
use crate::xray_op::actions::create_users;
use crate::xray_op::actions::remove_users;
use crate::xray_op::client::HandlerClient;

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
    pub trial: bool,
    pub limit: i64,
    pub password: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_message = serde_json::to_string(self).expect("Failed to serialize message");

        write!(f, "{}", json_message)
    }
}

impl Message {
    pub async fn handle_sub_message<T>(
        &self,
        client: Arc<Mutex<HandlerClient>>,
        state: Arc<Mutex<State<T>>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        T: Send + Sync + Clone + NodeStorage + 'static,
    {
        info!("Handling SUB message: {:?}", self);

        match self.process_message(client, state).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to process message: {}", e);
                Err(e)
            }
        }
    }
    pub async fn process_message<T>(
        &self,
        client: Arc<Mutex<HandlerClient>>,
        state: Arc<Mutex<State<T>>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        T: Send + Sync + Clone + NodeStorage + 'static,
    {
        match self.action {
            Action::Create | Action::Update => {
                let user_id = self.user_id;

                let user = User::new(
                    self.trial,
                    self.limit,
                    self.env.clone(),
                    self.password.clone(),
                );

                debug!("USER {:?}", user);

                match create_users(self.user_id, user.password.clone(), client).await {
                    Ok(_) => {
                        let mut state = state.lock().await;

                        state
                            .add_or_update_user(user_id.clone(), user)
                            .map_err(|err| {
                                error!("Failed to add user {}: {:?}", self.user_id, err);
                                format!("Failed to add user {}", self.user_id).into()
                            })
                    }
                    Err(err) => {
                        error!("Failed to create user {}: {:?}", self.user_id, err);
                        Err(format!("Failed to create user {}", self.user_id).into())
                    }
                }
            }
            Action::Delete => {
                if let Err(e) = remove_users(self.user_id, client).await {
                    return Err(format!("Couldn't remove users from Xray: {}", e).into());
                } else {
                    let mut state = state.lock().await;
                    let _ = state.remove_user(self.user_id);
                }

                Ok(())
            }
        }
    }
}
