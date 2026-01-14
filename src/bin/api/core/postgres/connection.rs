use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use defguard_wireguard_rs::net::IpAddrMask;
use pony::ConnectionBaseOp;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::Connection as Conn;
use pony::ConnectionStat;
use pony::Proto;
use pony::Tag;
use pony::WgKeys;
use pony::WgParam;

use pony::{PonyError, Result};

use super::PgClientManager;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConnRow {
    pub conn_id: uuid::Uuid,
    pub password: Option<String>,
    pub env: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub expired_at: Option<DateTime<Utc>>,
    pub subscription_id: Option<uuid::Uuid>,
    pub stat: ConnectionStat,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
    pub proto: Tag,
    is_deleted: bool,
}

impl From<(uuid::Uuid, Conn)> for ConnRow {
    fn from((conn_id, conn): (uuid::Uuid, Conn)) -> Self {
        let conn_stat = ConnectionStat {
            online: conn.stat.online,
            uplink: conn.stat.uplink,
            downlink: conn.stat.downlink,
        };

        ConnRow {
            conn_id: conn_id,
            password: conn.get_password(),
            env: conn.env.clone(),
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            expired_at: conn.expired_at,
            subscription_id: conn.subscription_id,
            stat: conn_stat,
            wg: conn.get_wireguard().cloned(),
            node_id: conn.get_wireguard_node_id(),
            proto: conn.get_proto().proto(),
            is_deleted: conn.is_deleted,
        }
    }
}

impl TryFrom<ConnRow> for Conn {
    type Error = PonyError;

    fn try_from(row: ConnRow) -> Result<Self> {
        let proto = match row.proto {
            Tag::Wireguard => {
                let wg = row
                    .wg
                    .ok_or_else(|| PonyError::Custom("Missing Wireguard param".into()))?;

                let node_id = row
                    .node_id
                    .ok_or_else(|| PonyError::Custom("Missing node_id".into()))?;

                Proto::new_wg(&wg, &node_id)
            }

            Tag::Shadowsocks => {
                let password = row
                    .password
                    .ok_or_else(|| PonyError::Custom("Missing Shadowsocks password".into()))?;

                Proto::new_ss(&password)
            }

            tag => Proto::new_xray(&tag),
        };

        Ok(Self {
            env: row.env,
            proto,
            stat: row.stat,
            subscription_id: row.subscription_id,
            created_at: row.created_at,
            modified_at: row.modified_at,
            expired_at: row.expired_at,
            is_deleted: row.is_deleted,
            node_id: row.node_id,
        })
    }
}

pub struct PgConn {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgConn {
    pub fn new(manager: Arc<Mutex<PgClientManager>>) -> Self {
        Self { manager }
    }

    pub async fn all(&self) -> Result<Vec<ConnRow>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = "
        SELECT 
            id,
            password,
            env,
            created_at,
            modified_at,
            expired_at,
            subscription_id,
            online, 
            uplink,
            downlink,
            proto,
            node_id,
            wg_privkey,
            wg_pubkey,
            wg_address,
            is_deleted
        FROM connections
    ";

        let rows = client.query(query, &[]).await?;
        let conns = self.map_rows_to_conns(rows);
        Ok(conns)
    }

    fn map_rows_to_conns(&self, rows: Vec<tokio_postgres::Row>) -> Vec<ConnRow> {
        rows.into_iter()
            .map(|row| {
                let conn_id: uuid::Uuid = row.get("id");
                let password: Option<String> = row.get("password");
                let env: String = row.get("env");
                let created_at: NaiveDateTime = row.get("created_at");
                let modified_at: NaiveDateTime = row.get("modified_at");
                let expired_at: Option<DateTime<Utc>> = row.get("expired_at");
                let subscription_id: Option<uuid::Uuid> = row.get("subscription_id");
                let online: i64 = row.get("online");
                let uplink: i64 = row.get("uplink");
                let downlink: i64 = row.get("downlink");
                let proto: Tag = row.get("proto");
                let node_id: Option<uuid::Uuid> = row.get("node_id");
                let wg_privkey: Option<String> = row.get("wg_privkey");
                let wg_pubkey: Option<String> = row.get("wg_pubkey");
                let wg_address: Option<String> = row.get("wg_address");
                let is_deleted: bool = row.get("is_deleted");

                let wg = match (wg_privkey, wg_pubkey, wg_address) {
                    (Some(privkey), Some(pubkey), Some(address)) => {
                        address.parse::<IpAddrMask>().ok().map(|ip_mask| WgParam {
                            keys: WgKeys { privkey, pubkey },
                            address: ip_mask.into(),
                        })
                    }
                    _ => None,
                };

                ConnRow {
                    conn_id,
                    password,
                    env,
                    created_at,
                    modified_at,
                    expired_at,
                    subscription_id,
                    stat: ConnectionStat {
                        online,
                        uplink,
                        downlink,
                    },
                    proto,
                    wg,
                    node_id,
                    is_deleted,
                }
            })
            .collect()
    }

    pub async fn update_stat(&self, conn_id: &uuid::Uuid, stat: ConnectionStat) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = "
                       UPDATE connections 
                       SET downlink = $1, uplink = $2, online = $3
                       WHERE id = $4";

        client
            .execute(
                query,
                &[&stat.downlink, &stat.uplink, &stat.online, &conn_id],
            )
            .await?;

        Ok(())
    }

    pub async fn delete(&self, conn_id: &uuid::Uuid) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = format!("UPDATE connections SET is_deleted = true WHERE id = $1");

        client.execute(&query, &[conn_id]).await?;

        Ok(())
    }

    pub async fn insert(&self, conn: ConnRow) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = "
        INSERT INTO connections (
            id,
            password,
            env,
            created_at,
            modified_at,
            expired_at,
            subscription_id,
            online,
            uplink,
            downlink,
            proto,
            is_deleted,
            wg_privkey,
            wg_pubkey,
            wg_address,
            node_id
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16
        )
    ";

        let result = client
            .execute(
                query,
                &[
                    &conn.conn_id,
                    &conn.password,
                    &conn.env,
                    &conn.created_at,
                    &conn.modified_at,
                    &conn.expired_at,
                    &conn.subscription_id,
                    &conn.stat.online,
                    &conn.stat.uplink,
                    &conn.stat.downlink,
                    &conn.proto,
                    &conn.is_deleted,
                    &conn.wg.as_ref().map(|w| &w.keys.privkey),
                    &conn.wg.as_ref().map(|w| &w.keys.pubkey),
                    &conn.wg.as_ref().map(|w| w.address.to_string()),
                    &conn.node_id,
                ],
            )
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(code) = e.code() {
                    if code == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
                        return Err(PonyError::Custom(format!(
                            "Connection {} already exists",
                            conn.conn_id
                        )));
                    }
                }

                Err(PonyError::Database(e))
            }
        }
    }
    pub async fn update(&self, conn: ConnRow) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = "
        UPDATE connections SET 
            password = $2,
            env = $3,
            modified_at = $4,
            subscription_id = $5,
            online = $6,
            uplink = $7,
            downlink = $8, 
            is_deleted = $9,
            wg_privkey = $10,
            wg_pubkey = $11,
            wg_address = $12,
            node_id = $13,
            expired_at = $14
        WHERE id = $1
    ";

        let rows = client
            .execute(
                query,
                &[
                    &conn.conn_id,
                    &conn.password,
                    &conn.env,
                    &conn.modified_at,
                    &conn.subscription_id,
                    &conn.stat.online,
                    &conn.stat.uplink,
                    &conn.stat.downlink,
                    &conn.is_deleted,
                    &conn.wg.as_ref().map(|w| &w.keys.privkey),
                    &conn.wg.as_ref().map(|w| &w.keys.pubkey),
                    &conn.wg.as_ref().map(|w| w.address.to_string()),
                    &conn.node_id,
                    &conn.expired_at,
                ],
            )
            .await?;

        if rows == 0 {
            return Err(PonyError::Custom("No rows updated".into()));
        }

        Ok(())
    }
}
