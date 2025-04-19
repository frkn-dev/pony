use anyhow::Result;
use chrono::Duration;
use chrono::NaiveDateTime;
use chrono::Utc;
use log;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SubscriptionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub payment_id: String,
    pub amount: f64,
    pub currency: String,
    pub created_at: NaiveDateTime,
    pub expired_at: NaiveDateTime,
}

pub struct PgSubscriptionRequest {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgSubscriptionRequest {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }
    pub async fn subscriptions_db_request(
        &self,
        user_id: Option<Uuid>,
    ) -> Result<Vec<SubscriptionRow>> {
        let client = self.client.lock().await;

        let query = if user_id.is_some() {
            "
        SELECT id, user_id, payment_id, amount, currency, created_at, expired_at 
        FROM subscriptions 
        WHERE user_id = $1
        "
        } else {
            "
        SELECT id, user_id, payment_id, amount, currency, created_at, expired_at 
        FROM subscriptions
        "
        };

        let rows = if let Some(uid) = user_id {
            client.query(query, &[&uid]).await?
        } else {
            client.query(query, &[]).await?
        };

        let subscriptions = rows
            .into_iter()
            .map(|row| SubscriptionRow {
                id: row.get(0),
                user_id: row.get(1),
                payment_id: row.get(2),
                amount: row.get(3),
                currency: row.get(4),
                created_at: row.get(5),
                expired_at: row.get(6),
            })
            .collect();

        Ok(subscriptions)
    }

    pub async fn subscription_exist(&self, user_id: Uuid) -> Option<NaiveDateTime> {
        let client = self.client.lock().await;

        let query = "
        SELECT expired_at 
        FROM subscriptions 
        WHERE user_id = $1
        ORDER BY expired_at DESC
        LIMIT 1
    ";

        match client.query(query, &[&user_id]).await {
            Ok(rows) => rows.first().map(|row| row.get(0)),
            Err(err) => {
                log::error!("Database query failed: {}", err);
                None
            }
        }
    }

    pub async fn insert_subscription(&self, subscription: SubscriptionRow) -> Result<()> {
        let expired_at = subscription.created_at + Duration::days(30);

        let client = self.client.lock().await;

        let query = "
        INSERT INTO subscriptions (id, user_id, payment_id, amount, currency, created_at, expired_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
    ";

        client
            .execute(
                query,
                &[
                    &subscription.id,
                    &subscription.user_id,
                    &subscription.payment_id,
                    &subscription.amount,
                    &subscription.currency,
                    &subscription.created_at,
                    &expired_at,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn add_subscription(
        &self,
        subscription: SubscriptionRow,
        existing_expired_at: Option<NaiveDateTime>,
    ) -> Result<()> {
        let new_expired_at = if let Some(current_expired_at) = existing_expired_at {
            if current_expired_at > Utc::now().naive_utc() {
                current_expired_at + Duration::days(30)
            } else {
                Utc::now().naive_utc() + Duration::days(30)
            }
        } else {
            Utc::now().naive_utc() + Duration::days(30)
        };

        let subscription = SubscriptionRow {
            expired_at: new_expired_at,
            ..subscription
        };

        self.insert_subscription(subscription).await
    }

    pub async fn has_active_subscription(&self, user_id: Uuid) -> Result<bool> {
        let client = self.client.lock().await;

        let query = "
        SELECT EXISTS(
            SELECT 1 FROM subscriptions
            WHERE user_id = $1 AND expired_at > $2
        )
    ";

        let now = Utc::now().naive_utc();
        let row = client.query_one(query, &[&user_id, &now]).await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }
}
