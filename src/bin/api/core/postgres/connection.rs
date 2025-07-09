use chrono::NaiveDateTime;
use defguard_wireguard_rs::net::IpAddrMask;
use pony::ConnectionBaseOp;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::Connection as Conn;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::Proto;
use pony::Tag;
use pony::WgKeys;
use pony::WgParam;

use pony::{PonyError, Result};

use super::PgClientManager;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConnRow {
    pub conn_id: uuid::Uuid,
    pub trial: bool,
    pub limit: i32,
    pub password: Option<String>,
    pub env: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub user_id: Option<uuid::Uuid>,
    pub stat: ConnectionStat,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
    pub status: ConnectionStatus,
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
            trial: conn.trial,
            limit: conn.limit,
            password: conn.get_password(),
            env: conn.env.clone(),
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            user_id: conn.user_id,
            stat: conn_stat,
            status: conn.status,
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
            trial: row.trial,
            limit: row.limit,
            env: row.env,
            proto,
            status: row.status,
            stat: row.stat,
            user_id: row.user_id,
            created_at: row.created_at,
            modified_at: row.modified_at,
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
            is_trial,
            daily_limit_mb,
            password,
            env,
            created_at,
            modified_at,
            user_id,
            online, 
            uplink,
            downlink,
            status,
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
                let conn_id: uuid::Uuid = row.get(0);
                let trial: bool = row.get(1);
                let limit: i32 = row.get(2);
                let password: Option<String> = row.get(3);
                let env: String = row.get(4);
                let created_at: NaiveDateTime = row.get(5);
                let modified_at: NaiveDateTime = row.get(6);
                let user_id: Option<uuid::Uuid> = row.get(7);
                let online: i64 = row.get(8);
                let uplink: i64 = row.get(9);
                let downlink: i64 = row.get(10);
                let status: ConnectionStatus = row.get(11);
                let proto: Tag = row.get(12);
                let node_id: Option<uuid::Uuid> = row.get(13);
                let wg_privkey: Option<String> = row.get(14);
                let wg_pubkey: Option<String> = row.get(15);
                let wg_address: Option<String> = row.get(16);
                let is_deleted: bool = row.get(17);

                let wg = match (wg_privkey, wg_pubkey, wg_address) {
                    (Some(privkey), Some(pubkey), Some(address)) => {
                        address.parse::<IpAddrMask>().ok().map(|ip_mask| WgParam {
                            keys: WgKeys { privkey, pubkey },
                            address: ip_mask,
                        })
                    }
                    _ => None,
                };

                ConnRow {
                    conn_id,
                    trial,
                    limit,
                    password,
                    env,
                    created_at,
                    modified_at,
                    user_id,
                    stat: ConnectionStat {
                        online,
                        uplink,
                        downlink,
                    },
                    status,
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

    pub async fn update_status(
        &self,
        conn_id: &uuid::Uuid,
        status: ConnectionStatus,
    ) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = format!("UPDATE connections SET status = $1::conn_status WHERE id = $2");

        client.execute(&query, &[&status, conn_id]).await?;

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
            is_trial,
            daily_limit_mb,
            password,
            env,
            created_at,
            modified_at,
            user_id,
            online,
            uplink,
            downlink,
            proto,
            status,
            is_deleted,
            wg_privkey,
            wg_pubkey,
            wg_address,
            node_id
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18
        )
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
            is_trial = $2,
            daily_limit_mb = $3,
            password = $4,
            env = $5,
            modified_at = $6,
            user_id = $7,
            online = $8,
            uplink = $9,
            downlink = $10, 
            status = $11,
            is_deleted = $12,
            wg_privkey = $13,
            wg_pubkey = $14,
            wg_address = $15,
            node_id = $16
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
                    &conn.is_deleted,
                    &conn.wg.as_ref().map(|w| &w.keys.privkey),
                    &conn.wg.as_ref().map(|w| &w.keys.pubkey),
                    &conn.wg.as_ref().map(|w| w.address.to_string()),
                    &conn.node_id,
                ],
            )
            .await?;

        if rows == 0 {
            return Err(PonyError::Custom("No rows updated".into()));
        }

        Ok(())
    }
}
