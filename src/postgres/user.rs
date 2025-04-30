use chrono::{NaiveDateTime, Utc};
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PgClient;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{PonyError, Result};

pub struct UserRow {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
}

pub struct PgUser {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgUser {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub async fn insert(&self, user_id: &uuid::Uuid, username: &str) -> Result<()> {
        let client = self.client.lock().await;
        let now = Utc::now().naive_utc();

        let query = "
        INSERT INTO users (id, username, created_at, modified_at)
        VALUES ($1, $2, $3, $4)
    ";

        let res = client
            .execute(query, &[&user_id, &username, &now, &now])
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(e) if e.code() == Some(&SqlState::UNIQUE_VIOLATION) => Err(PonyError::Conflict),
            Err(e) => Err(PonyError::Database(e)),
        }
    }

    pub async fn get(&self, username: &str) -> Option<UserRow> {
        let client = self.client.lock().await;

        let query = "SELECT id, username, created_at, modified_at FROM users WHERE username = $1;";
        let result = client.query(query, &[&username]).await;

        if let Ok(rows) = result {
            if let Some(row) = rows.first() {
                return Some(UserRow {
                    user_id: row.get("id"),
                    username: row.get("username"),
                    created_at: row.get("created_at"),
                    modified_at: row.get("modified_at"),
                });
            }
        }

        None
    }
}
