use chrono::Utc;
use tokio_postgres::Client as PgClient;

use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct PgUser {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgUser {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub async fn insert(
        &self,
        user_id: &uuid::Uuid,
        username: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        let now = Utc::now().naive_utc();

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
