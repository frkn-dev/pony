use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PgClient;

use crate::state::connection::Conn;
use crate::state::ConnStat;
use crate::state::ConnStatus;
use crate::state::Tag;
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
    pub stat: ConnStat,
    pub status: ConnStatus,
    pub proto: Tag,
    is_deleted: bool,
}

impl From<(uuid::Uuid, Conn)> for ConnRow {
    fn from((conn_id, conn): (uuid::Uuid, Conn)) -> Self {
        let conn_stat = ConnStat {
            online: conn.stat.online,
            uplink: conn.stat.uplink,
            downlink: conn.stat.downlink,
        };

        ConnRow {
            conn_id: conn_id,
            trial: conn.trial,
            limit: conn.limit,
            password: conn.password.unwrap_or_default(),
            env: conn.env,
            created_at: conn.created_at.naive_utc(),
            modified_at: conn.modified_at.naive_utc(),
            user_id: conn.user_id,
            stat: conn_stat,
            status: conn.status,
            proto: conn.proto,
            is_deleted: conn.is_deleted,
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
        SELECT id, is_trial, daily_limit_mb, password, env, created_at, modified_at, user_id, online, uplink, downlink, status, proto, is_deleted 
        FROM connections
    ";

        let rows = client.query(query, &[]).await?;
        let conns = self.map_rows_to_conns(rows);
        Ok(conns)
    }

    pub async fn by_env(&self, env: &str) -> Result<Vec<ConnRow>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, is_trial, daily_limit_mb, password, env, created_at, modified_at, user_id, online, uplink, downlink, status, proto, is_deleted
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
                let conn_id: uuid::Uuid = row.get(0);
                let trial: bool = row.get(1);
                let limit: i32 = row.get(2);
                let password: String = row.get(3);
                let env: String = row.get(4);
                let created_at: NaiveDateTime = row.get(5);
                let modified_at: NaiveDateTime = row.get(6);
                let user_id: Option<uuid::Uuid> = row.get(7);
                let online: i64 = row.get(8);
                let uplink: i64 = row.get(9);
                let downlink: i64 = row.get(10);
                let status: ConnStatus = row.get(11);
                let proto: Tag = row.get(12);
                let is_deleted: bool = row.get(13);

                ConnRow {
                    conn_id,
                    trial,
                    limit,
                    password,
                    env,
                    created_at,
                    modified_at,
                    user_id,
                    stat: ConnStat {
                        online: online,
                        uplink: uplink,
                        downlink: downlink,
                    },
                    status: status,
                    proto: proto,
                    is_deleted: is_deleted,
                }
            })
            .collect()
    }

    pub async fn is_exist(&self, conn_id: &str) -> Option<uuid::Uuid> {
        let client = self.client.lock().await;

        let query = "
                        SELECT id 
                        FROM connections 
                        WHERE conn_id = $1";

        match client.query(query, &[&conn_id]).await {
            Ok(rows) => rows.first().map(|row| row.get(0)),
            Err(err) => {
                log::error!("Database query failed: {}", err);
                None
            }
        }
    }

    pub async fn update_stat(&self, conn_id: &uuid::Uuid, stat: ConnStat) -> Result<()> {
        let query = "
                       UPDATE connections 
                       SET downlink = $1, uplink = $2, online = $3
                       WHERE id = $4";

        let client = self.client.lock().await;
        client
            .execute(
                query,
                &[&stat.downlink, &stat.uplink, &stat.online, &conn_id],
            )
            .await?;

        Ok(())
    }

    pub async fn update_status(&self, conn_id: &uuid::Uuid, status: ConnStatus) -> Result<()> {
        let query = format!("UPDATE connections SET status = $1::conn_status WHERE id = $2");

        let client = self.client.lock().await;
        client.execute(&query, &[&status, conn_id]).await?;

        Ok(())
    }

    pub async fn delete(&self, conn_id: &uuid::Uuid) -> Result<()> {
        let query = format!("UPDATE connections SET is_deleted = true WHERE id = $1");

        let client = self.client.lock().await;
        client.execute(&query, &[conn_id]).await?;

        Ok(())
    }

    pub async fn insert(&self, conn: ConnRow) -> Result<()> {
        let client = self.client.lock().await;

        let query = "
        INSERT INTO connections (id, is_trial, daily_limit_mb, password, env, created_at, modified_at, user_id, online, uplink, downlink, proto, status, is_deleted )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
                    &conn.stat.online,
                    &conn.stat.uplink,
                    &conn.stat.downlink,
                    &conn.proto,
                    &conn.status,
                    &conn.is_deleted,
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

    pub async fn update(&self, conn: ConnRow) -> Result<()> {
        let client = self.client.lock().await;

        let query = "
                    UPDATE connections SET 
                        is_trial = $2,
                        daily_limit_mb = $3,
                        password = $4,
                        env = $5,
                        modified_at = $6,
                        user_id = $7,
                        online = $8,
                        uplink = $9,
                        downlink = $10, 
                        status = $11
                    WHERE id = $1
                   ";

        let rows = client
            .execute(
                query,
                &[
                    &conn.conn_id,
                    &conn.trial,
                    &conn.limit,
                    &conn.password,
                    &conn.env,
                    &conn.modified_at,
                    &conn.user_id,
                    &conn.stat.online,
                    &conn.stat.uplink,
                    &conn.stat.downlink,
                    &conn.status,
                ],
            )
            .await?;

        if rows == 0 {
            return Err(PonyError::Custom("No rows updated".into()).into());
        }

        Ok(())
    }
}
