use chrono::NaiveDateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use uuid::Uuid;

use crate::zmq::message::Action;
use crate::zmq::message::Message as ConnRequest;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConnRow {
    pub conn_id: Uuid,
    pub trial: bool,
    pub limit: i64,
    pub password: String,
    pub env: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
}

impl ConnRow {
    /// ToDo: Fix
    pub fn new(conn_id: Uuid) -> Self {
        let now = Utc::now().naive_utc();
        Self {
            conn_id: conn_id,
            trial: true,
            limit: 1000,
            password: "password".to_string(),
            env: "dev".to_string(),
            created_at: now,
            modified_at: now,
        }
    }

    pub fn as_create_conn_request(&self) -> ConnRequest {
        ConnRequest {
            conn_id: self.conn_id,
            action: Action::Create,
            env: self.env.clone(),
            trial: self.trial,
            limit: self.limit,
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

    pub async fn all(&self) -> Result<Vec<ConnRow>, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, is_trial, daily_limit_mb, password, env, created_at, modified_at 
        FROM connections
    ";

        let rows = client.query(query, &[]).await?;
        let conns = self.map_rows_to_conns(rows);
        Ok(conns)
    }

    pub async fn by_env(&self, env: &str) -> Result<Vec<ConnRow>, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, trial, daily_limit_mb, password, env, created_at, modified_at 
        FROM connections
        WHERE env = $1
    ";

        let rows = client.query(query, &[&env]).await?;

        let conns = self.map_rows_to_conns(rows);
        Ok(conns)
    }

    fn map_rows_to_conns(&self, rows: Vec<tokio_postgres::Row>) -> Vec<ConnRow> {
        rows.into_iter()
            .map(|row| {
                let conn_id: Uuid = row.get(0);
                let trial: bool = row.get(1);
                let limit: i64 = row.get(2);
                let password: String = row.get(3);
                let env: String = row.get(4);
                let created_at: NaiveDateTime = row.get(5);
                let modified_at: NaiveDateTime = row.get(6);

                ConnRow {
                    conn_id,
                    trial,
                    limit,
                    password,
                    env,
                    created_at,
                    modified_at,
                }
            })
            .collect()
    }

    pub async fn is_exist(&self, conn_id: String) -> Option<Uuid> {
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

    pub async fn insert(&self, conn: ConnRow) -> Result<(), Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = "
        INSERT INTO connections (id, is_trial, daily_limit_mb, password, env, created_at, modified_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
    ";

        client
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
                ],
            )
            .await?;

        Ok(())
    }
}
