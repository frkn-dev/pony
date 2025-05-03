use chrono::NaiveDateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PgClient;

use crate::state::connection::Conn;
use crate::zmq::message::Action;
use crate::zmq::message::Message as ConnRequest;
use crate::{PonyError, Result};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConnRow {
    pub conn_id: uuid::Uuid,
    pub trial: bool,
    pub limit: i32,
    pub password: String,
    pub env: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub user_id: Option<uuid::Uuid>,
}

impl From<(uuid::Uuid, Conn)> for ConnRow {
    fn from((conn_id, conn): (uuid::Uuid, Conn)) -> Self {
        ConnRow {
            conn_id: conn_id,
            trial: conn.trial,
            limit: conn.limit,
            password: conn.password.unwrap_or_default(),
            env: conn.env,
            created_at: conn.created_at.naive_utc(),
            modified_at: conn.modified_at.naive_utc(),
            user_id: conn.user_id,
        }
    }
}

impl ConnRow {
    /// ToDo: Fix
    pub fn new(conn_id: uuid::Uuid) -> Self {
        let now = Utc::now().naive_utc();
        Self {
            conn_id: conn_id,
            trial: true,
            limit: 1000,
            password: "password".to_string(),
            env: "dev".to_string(),
            created_at: now,
            modified_at: now,
            user_id: None,
        }
    }

    pub fn as_create_conn_request(&self) -> ConnRequest {
        ConnRequest {
            conn_id: self.conn_id,
            action: Action::Create,
            password: Some(self.password.clone()),
        }
    }
}

pub struct PgConn {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgConn {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub async fn all(&self) -> Result<Vec<ConnRow>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, is_trial, daily_limit_mb, password, env, created_at, modified_at, user_id 
        FROM connections
    ";

        let rows = client.query(query, &[]).await?;
        let conns = self.map_rows_to_conns(rows);
        Ok(conns)
    }

    pub async fn by_env(&self, env: &str) -> Result<Vec<ConnRow>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, is_trial, daily_limit_mb, password, env, created_at, modified_at, user_id 
        FROM connections
        WHERE env = $1 or env = 'all'
    ";

        let rows = client.query(query, &[&env]).await?;

        let conns = self.map_rows_to_conns(rows);
        Ok(conns)
    }

    fn map_rows_to_conns(&self, rows: Vec<tokio_postgres::Row>) -> Vec<ConnRow> {
        rows.into_iter()
            .map(|row| {
                let conn_id: uuid::Uuid = row.get(0);
                let trial: bool = row.get(1);
                let limit: i32 = row.get(2);
                let password: String = row.get(3);
                let env: String = row.get(4);
                let created_at: NaiveDateTime = row.get(5);
                let modified_at: NaiveDateTime = row.get(6);
                let user_id: Option<uuid::Uuid> = row.get(7);

                ConnRow {
                    conn_id,
                    trial,
                    limit,
                    password,
                    env,
                    created_at,
                    modified_at,
                    user_id,
                }
            })
            .collect()
    }

    pub async fn is_exist(&self, conn_id: &str) -> Option<uuid::Uuid> {
        let client = self.client.lock().await;

        let query = "
        SELECT id 
        FROM connections 
        WHERE conn_id = $1
    ";

        match client.query(query, &[&conn_id]).await {
            Ok(rows) => rows.first().map(|row| row.get(0)),
            Err(err) => {
                log::error!("Database query failed: {}", err);
                None
            }
        }
    }

    pub async fn insert(&self, conn: ConnRow) -> Result<()> {
        let client = self.client.lock().await;

        let query = "
        INSERT INTO connections (id, is_trial, daily_limit_mb, password, env, created_at, modified_at, user_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    ";

        let result = client
            .execute(
                query,
                &[
                    &conn.conn_id,
                    &conn.trial,
                    &conn.limit,
                    &conn.password,
                    &conn.env,
                    &conn.created_at,
                    &conn.modified_at,
                    &conn.user_id,
                ],
            )
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(code) = e.code() {
                    if code == &SqlState::UNIQUE_VIOLATION {
                        return Err(PonyError::Custom(
                            format!("Connection {} already exists", conn.conn_id).into(),
                        ));
                    }
                }

                Err(PonyError::Database(e))
            }
        }
    }
}
