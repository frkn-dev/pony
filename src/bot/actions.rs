use async_trait::async_trait;
use std::error::Error;

use super::BotState;
use crate::ConnRow;

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
        Ok(())
    }
}
