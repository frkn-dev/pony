use std::error::Error;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;

use crate::state::node::Node;
use crate::state::node::NodeStatus;
use crate::utils::to_ipv4;

pub struct PgNode {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgNode {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub async fn insert(&self, node: Node) -> Result<(), Box<dyn Error>> {
        let client = self.client.lock().await;

        let query = "
            INSERT INTO nodes (uuid, env, hostname, address, status, inbounds, created_at, modified_at, label, interface)
            VALUES ($1, $2, $3, $4, $5::node_status, $6, $7, $8, $9, $10)
        ";

        let address: IpAddr = IpAddr::V4(node.address);
        let inbounds_json = serde_json::to_value(&node.inbounds)?;

        let result = client
            .execute(
                query,
                &[
                    &node.uuid,
                    &node.env,
                    &node.hostname,
                    &address,
                    &node.status,
                    &inbounds_json,
                    &node.created_at,
                    &node.modified_at,
                    &node.label,
                    &node.interface,
                ],
            )
            .await;
        match result {
            Ok(_) => log::debug!("Node inserted successfully"),
            Err(e) => log::error!("Error inserting node: {}", e),
        }

        Ok(())
    }

    pub async fn all(&self) -> Result<Vec<Node>, Box<dyn Error>> {
        let client = self.client.lock().await;

        let rows = client
        .query(
            "SELECT uuid, env, hostname, address, status, inbounds, created_at, modified_at, label, interface 
             FROM nodes",
            &[],
        )
        .await?;

        let nodes: Vec<Node> = rows
            .into_iter()
            .filter_map(|row| {
                let uuid: uuid::Uuid = row.get(0);
                let env: String = row.get(1);
                let hostname: String = row.get(2);
                let address: IpAddr = row.get(3);
                let status: NodeStatus = row.get(4);
                let inbounds: Value = row.get(5);
                let created_at: DateTime<Utc> = row.get(6);
                let modified_at: DateTime<Utc> = row.get(7);
                let label: String = row.get(8);
                let interface: String = row.get(9);

                if let Some(ipv4_addr) = to_ipv4(address) {
                    Some(Node {
                        uuid: uuid,
                        env: env,
                        hostname: hostname,
                        address: ipv4_addr,
                        interface: interface,
                        status: status,
                        inbounds: serde_json::from_value(inbounds).unwrap_or_default(),
                        created_at,
                        modified_at,
                        label: label,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(nodes)
    }

    pub async fn update_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        new_status: NodeStatus,
    ) -> Result<(), Box<dyn Error>> {
        let client = self.client.lock().await;

        let query = "
            UPDATE nodes
            SET status = $1::node_status, modified_at = $2
            WHERE uuid = $3 AND env = $4
        ";
        let modified_at = Utc::now();

        let result = client
            .execute(query, &[&new_status, &modified_at, &uuid, &env])
            .await;

        match result {
            Ok(rows_updated) => {
                if rows_updated == 0 {
                    log::warn!("No node found with UUID {}", uuid);
                } else {
                    log::debug!("Updated node {} status to {}", uuid, new_status);
                }
            }
            Err(e) => {
                log::error!("Error updating node status: {}", e);
                return Err(e.into());
            }
        }

        Ok(())
    }
}
