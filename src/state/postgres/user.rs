use chrono::NaiveDateTime;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PgClient;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{PonyError, Result};

use super::super::user::User;
use crate::utils::to_pg_bigint;

pub struct UserRow {
    pub user_id: uuid::Uuid,
    pub telegram_id: Option<i64>,
    pub username: String,
    pub env: String,
    pub limit: Option<i32>,
    pub password: Option<String>,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub is_deleted: bool,
}

impl From<(uuid::Uuid, User)> for UserRow {
    fn from((user_id, user): (uuid::Uuid, User)) -> Self {
        let tg_id = if let Some(id) = user.telegram_id {
            to_pg_bigint(id)
        } else {
            None
        };
        UserRow {
            user_id: user_id,
            username: user.username,
            telegram_id: tg_id,
            created_at: user.created_at,
            modified_at: user.modified_at,
            env: user.env,
            limit: user.limit,
            password: user.password,
            is_deleted: user.is_deleted,
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
        INSERT INTO users (id, username, telegram_id, env, daily_limit_mb, password, created_at, modified_at, is_deleted)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    ";

        let res = client
            .execute(
                query,
                &[
                    &user_row.user_id,
                    &user_row.username,
                    &user_row.telegram_id,
                    &user_row.env,
                    &user_row.limit,
                    &user_row.password,
                    &user_row.created_at,
                    &user_row.modified_at,
                    &user_row.is_deleted,
                ],
            )
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(e) if e.code() == Some(&SqlState::UNIQUE_VIOLATION) => Err(PonyError::Conflict),
            Err(e) => Err(PonyError::Database(e)),
        }
    }

    pub async fn delete(&self, user_id: &uuid::Uuid) -> Result<()> {
        let client = self.client.lock().await;

        let _ = client
            .execute(
                "UPDATE users SET is_deleted = true WHERE id = $1",
                &[user_id],
            )
            .await?;

        Ok(())
    }
    pub async fn update(&self, user: UserRow) -> Result<()> {
        let client = self.client.lock().await;

        let query = "
                    UPDATE users SET telegram_id = $2, env = $3, daily_limit_mb = $4, password = $5,  modified_at = $6, is_deleted = $7
                    WHERE username = $1
                   ";

        let rows = client
            .execute(
                query,
                &[
                    &user.username,
                    &user.telegram_id,
                    &user.env,
                    &user.limit,
                    &user.password,
                    &user.modified_at,
                    &user.is_deleted,
                ],
            )
            .await?;

        if rows == 0 {
            return Err(PonyError::Custom("No rows updated".into()).into());
        }

        Ok(())
    }

    pub async fn all(&self) -> Result<Vec<UserRow>> {
        let client = self.client.lock().await;

        let rows = client
            .query(
                "SELECT id, username, telegram_id, env, daily_limit_mb, password, created_at, modified_at, is_deleted FROM users",
                &[],
            )
            .await?;

        let users: Vec<UserRow> = rows
            .into_iter()
            .filter_map(|row| {
                let uuid: uuid::Uuid = row.get(0);
                let username: String = row.get(1);
                let telegram_id: Option<i64> = row.get(2);
                let env: String = row.get(3);
                let limit: Option<i32> = row.get(4);
                let password: Option<String> = row.get(5);
                let created_at: NaiveDateTime = row.get(6);
                let modified_at: NaiveDateTime = row.get(7);
                let is_deleted: bool = row.get(8);

                Some(UserRow {
                    user_id: uuid,
                    username,
                    telegram_id,
                    env,
                    limit,
                    password,
                    created_at,
                    modified_at,
                    is_deleted,
                })
            })
            .collect();

        Ok(users)
    }

    pub async fn get(&self, username: &str) -> Option<UserRow> {
        let client = self.client.lock().await;

        let query = "SELECT id, username, telegram_id, env, daily_limit_mb, password, created_at, modified_at, is_deleted FROM users WHERE username = $1;";
        let result = client.query(query, &[&username]).await;

        if let Ok(rows) = result {
            if let Some(row) = rows.first() {
                return Some(UserRow {
                    user_id: row.get("id"),
                    username: row.get("username"),
                    telegram_id: row.get("telegram_id"),
                    env: row.get("env"),
                    limit: row.get("daily_limit_mb"),
                    password: row.get("password"),
                    created_at: row.get("created_at"),
                    modified_at: row.get("modified_at"),
                    is_deleted: row.get("is_deleted"),
                });
            }
        }

        None
    }
}
