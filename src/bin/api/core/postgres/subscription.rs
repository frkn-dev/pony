use std::sync::Arc;
use tokio::sync::Mutex;

use super::PgClientManager;
use pony::memory::subscription::Subscription;
use pony::Result;

pub struct PgSubscription {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgSubscription {
    pub fn new(manager: Arc<Mutex<PgClientManager>>) -> Self {
        Self { manager }
    }

    pub async fn all(&self) -> Result<Vec<Subscription>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                "SELECT * FROM subscriptions WHERE NOT is_deleted ORDER BY created_at DESC",
                &[],
            )
            .await?;

        let subscriptions: Vec<Subscription> = rows.into_iter().map(Subscription::from).collect();

        Ok(subscriptions)
    }

    pub async fn create(&self, new_sub: &Subscription) -> Result<Subscription> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_one(
                r#"
            INSERT INTO subscriptions 
            (id, expires_at, referred_by)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
                &[&new_sub.id, &new_sub.expires_at, &new_sub.referred_by],
            )
            .await?;

        Ok(Subscription::from(row))
    }

    pub async fn update_expires_at(
        &self,
        id: uuid::Uuid,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Subscription> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let now = chrono::Utc::now();

        let row = client
            .query_one(
                r#"
            UPDATE subscriptions
            SET expires_at = $1,
                updated_at = $2
            WHERE id = $3
            RETURNING *
            "#,
                &[&expires_at, &now, &id],
            )
            .await?;

        Ok(Subscription::from(row))
    }
}
