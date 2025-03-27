use anyhow::Result;
use chrono::NaiveDateTime;
use log;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SubscriptionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub payment_id: String,
    pub amount: f64,
    pub currency: String,
    pub created_at: NaiveDateTime,
}

pub async fn subscriptions_db_request(
    client: Arc<Mutex<Client>>,
    user_id: Option<Uuid>,
) -> Result<Vec<SubscriptionRow>> {
    let client = client.lock().await;

    let query = if user_id.is_some() {
        "
        SELECT id, user_id, payment_id, amount, currency, created_at 
        FROM subscriptions 
        WHERE user_id = $1
        "
    } else {
        "
        SELECT id, user_id, payment_id, amount, currency, created_at 
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
        })
        .collect();

    Ok(subscriptions)
}

pub async fn subscription_exist(client: Arc<Mutex<Client>>, payment_id: String) -> Option<Uuid> {
    let client = client.lock().await;

    let query = "
        SELECT id 
        FROM subscriptions 
        WHERE payment_id = $1
    ";

    match client.query(query, &[&payment_id]).await {
        Ok(rows) => rows.first().map(|row| row.get(0)),
        Err(err) => {
            log::error!("Database query failed: {}", err);
            None
        }
    }
}

pub async fn insert_subscription(
    client: Arc<Mutex<Client>>,
    subscription: SubscriptionRow,
) -> Result<()> {
    let client = client.lock().await;

    let query = "
        INSERT INTO subscriptions (id, user_id, payment_id, amount, currency, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
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
            ],
        )
        .await?;

    Ok(())
}
