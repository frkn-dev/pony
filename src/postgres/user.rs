use crate::postgres::Client;
use chrono::Utc;

use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct PgUser {
    pub client: Arc<Mutex<Client>>,
}

impl PgUser {
    pub fn new(client: Arc<Mutex<Client>>) -> Self {
        Self { client }
    }

    pub async fn insert(&self, username: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        let user_id = uuid::Uuid::new_v4();
        let now = Utc::now();

        let query = "
        INSERT INTO users (id, username, created_at, modified_at)
        VALUES ($1, $2, $3, $4)
    ";

        client
            .execute(query, &[&user_id, &username, &now, &now])
            .await?;

        Ok(())
    }
}
