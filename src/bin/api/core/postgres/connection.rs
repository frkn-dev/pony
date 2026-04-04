use chrono::DateTime;
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
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub subscription_id: Option<uuid::Uuid>,
    pub stat: ConnectionStat,
    pub wg: Option<WgParam>,
    pub node_id: Option<uuid::Uuid>,
    pub proto: Tag,
    pub token: Option<uuid::Uuid>,
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
            conn_id,
            password: conn.get_password(),
            env: conn.env.clone(),
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            expires_at: conn.expires_at,
            subscription_id: conn.subscription_id,
            stat: conn_stat,
            wg: conn.get_wireguard().cloned(),
            node_id: conn.get_wireguard_node_id(),
            proto: conn.get_proto().proto(),
            token: conn.get_token(),
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

            Tag::Hysteria2 => {
                let token = row
                    .token
                    .ok_or_else(|| PonyError::Custom("Missing Hysteria2 token".into()))?;
                Proto::new_hysteria2(&token)
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
            expires_at: row.expires_at,
            is_deleted: row.is_deleted,
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
            token,
            env,
            created_at,
            modified_at,
            expires_at,
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
                let created_at: DateTime<Utc> = row.get("created_at");
                let modified_at: DateTime<Utc> = row.get("modified_at");
                let expires_at: Option<DateTime<Utc>> = row.get("expires_at");
                let subscription_id: Option<uuid::Uuid> = row.get("subscription_id");
                let token: Option<uuid::Uuid> = row.get("token");
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
                    token,
                    env,
                    created_at,
                    modified_at,
                    expires_at,
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

        let query = "UPDATE connections SET is_deleted = true WHERE id = $1";

        client.execute(query, &[conn_id]).await?;

        Ok(())
    }

    pub async fn restore(&self, conn_id: &uuid::Uuid) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = "UPDATE connections SET is_deleted = false WHERE id = $1";

        client.execute(query, &[conn_id]).await?;

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
            expires_at,
            subscription_id,
            online,
            uplink,
            downlink,
            proto,
            is_deleted,
            wg_privkey,
            wg_pubkey,
            wg_address,
            node_id,
            token
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17
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
                    &conn.expires_at,
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
                    &conn.token,
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
}
