use async_trait::async_trait;
use std::error::Error;

use super::BotState;
use crate::UserRow;

#[async_trait]
pub trait Actions {
    async fn register(
        &self,
        username: &str,
        user_id: uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
impl Actions for BotState {
    async fn register(
        &self,
        username: &str,
        user_id: uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(_user) = self.db.user().user_exist(username.to_string()).await {
            Err("User already exist".into())
        } else {
            log::info!("Registering user");
            let user = UserRow::new(username, user_id);
            self.db.user().insert_user(user).await
        }
    }
}
