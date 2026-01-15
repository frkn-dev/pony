use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::PgClientManager;
use pony::memory::subscription::NewSubscription;
use pony::memory::subscription::Subscription;
use pony::memory::subscription::UpdateSubscription;
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

    pub async fn create(&self, new_sub: &NewSubscription) -> Result<Subscription> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_one(
                r#"
            INSERT INTO subscriptions 
            (expires_at, referral_code, referred_by)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
                &[
                    &new_sub.expires_at,
                    &new_sub.referral_code,
                    &new_sub.referred_by,
                ],
            )
            .await?;

        Ok(Subscription::from(row))
    }

    pub async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<Subscription>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                "SELECT * FROM subscriptions WHERE id = $1 AND NOT is_deleted",
                &[&id],
            )
            .await?;

        Ok(row.map(Subscription::from))
    }

    pub async fn find_by_referral_code(&self, code: &str) -> Result<Option<Subscription>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                "SELECT * FROM subscriptions WHERE referral_code = $1 AND NOT is_deleted",
                &[&code],
            )
            .await?;

        Ok(row.map(Subscription::from))
    }

    pub async fn update(
        &self,
        id: uuid::Uuid,
        update: &UpdateSubscription,
    ) -> Result<Subscription> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        if update.expires_at.is_none()
            && update.referral_count.is_none()
            && update.referral_bonus_days.is_none()
            && update.is_deleted.is_none()
        {
            return self
                .find_by_id(id)
                .await?
                .ok_or_else(|| pony::PonyError::Custom("NO_DATA_FOUND".to_string()));
        }

        let expires_at = update.expires_at;
        let referral_count = update.referral_count;
        let referral_bonus_days = update.referral_bonus_days;
        let is_deleted = update.is_deleted;
        let now = Utc::now();

        let mut query_parts = Vec::new();
        let mut values: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();

        if let Some(exp) = &expires_at {
            query_parts.push("expires_at = $1");
            values.push(exp);
        }

        if let Some(rc) = &referral_count {
            query_parts.push("referral_count = $2");
            values.push(rc);
        }

        if let Some(rbd) = &referral_bonus_days {
            query_parts.push("referral_bonus_days = $3");
            values.push(rbd);
        }

        if let Some(del) = &is_deleted {
            query_parts.push("is_deleted = $4");
            values.push(del);
        }

        query_parts.push("updated_at = $5");
        values.push(&now);

        values.push(&id);

        let set_clause = query_parts.join(", ");
        let query = format!(
            "UPDATE subscriptions SET {} WHERE id = $6 RETURNING *",
            set_clause
        );

        let row = client.query_one(&query, &values).await?;
        Ok(Subscription::from(row))
    }

    pub async fn find_expired(&self) -> Result<Vec<Subscription>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                "SELECT * FROM subscriptions WHERE expires_at < NOW() AND NOT is_deleted",
                &[],
            )
            .await?;

        Ok(rows.into_iter().map(Subscription::from).collect())
    }
}
