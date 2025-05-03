use chrono::NaiveDateTime;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PgClient;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{PonyError, Result};

use super::super::user::User;

pub struct UserRow {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
}

impl From<(uuid::Uuid, User)> for UserRow {
    fn from((user_id, user): (uuid::Uuid, User)) -> Self {
        UserRow {
            user_id: user_id,
            username: user.username,
            created_at: user.created_at,
            modified_at: user.modified_at,
        }
    }
}

pub struct PgUser {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgUser {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub async fn insert(&self, user_row: UserRow) -> Result<()> {
        let client = self.client.lock().await;

        let query = "
        INSERT INTO users (id, username, created_at, modified_at)
        VALUES ($1, $2, $3, $4)
    ";

        let res = client
            .execute(
                query,
                &[
                    &user_row.user_id,
                    &user_row.username,
                    &user_row.created_at,
                    &user_row.modified_at,
                ],
            )
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(e) if e.code() == Some(&SqlState::UNIQUE_VIOLATION) => Err(PonyError::Conflict),
            Err(e) => Err(PonyError::Database(e)),
        }
    }

    pub async fn all(&self) -> Result<Vec<UserRow>> {
        let client = self.client.lock().await;

        let rows = client
            .query(
                "SELECT id, username, created_at, modified_at FROM users",
                &[],
            )
            .await?;

        let users: Vec<UserRow> = rows
            .into_iter()
            .filter_map(|row| {
                let uuid: uuid::Uuid = row.get(0);
                let username: String = row.get(1);
                let created_at: NaiveDateTime = row.get(2);
                let modified_at: NaiveDateTime = row.get(3);

                Some(UserRow {
                    user_id: uuid,
                    username,
                    created_at,
                    modified_at,
                })
            })
            .collect();

        Ok(users)
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
