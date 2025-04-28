use std::error::Error;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use log::debug;
use log::error;
use log::warn;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::state::node::Node;
use crate::state::node::NodeStatus;

fn to_ipv4(ip: IpAddr) -> Option<Ipv4Addr> {
    match ip {
        IpAddr::V4(ipv4) => Some(ipv4),
        IpAddr::V6(_) => None,
    }
}

pub struct PgNode {
    pub client: Arc<Mutex<Client>>,
}

impl PgNode {
    pub fn new(client: Arc<Mutex<Client>>) -> Self {
        Self { client }
    }

    pub async fn insert_node(&self, node: Node) -> Result<(), Box<dyn Error>> {
        let client = self.client.lock().await;

        let query = "
            INSERT INTO nodes (uuid, env, hostname, address, status, inbounds, created_at, modified_at, label)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ";

        let status_str = node.status.to_string();
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
                    &status_str,
                    &inbounds_json,
                    &node.created_at,
                    &node.modified_at,
                    &node.label,
                ],
            )
            .await;
        match result {
            Ok(_) => debug!("Node inserted successfully"),
            Err(e) => error!("Error inserting node: {}", e),
        }

        Ok(())
    }

    pub async fn get_nodes(&self) -> Result<Vec<Node>, Box<dyn Error>> {
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
                let uuid: Uuid = row.get(0);
                let env: String = row.get(1);
                let hostname: String = row.get(2);
                let address: IpAddr = row.get(3);
                let status: String = row.get(4);
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
                        status: NodeStatus::from_str(&status).unwrap_or(NodeStatus::Offline),
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

    pub async fn update_node_status(
        &self,
        uuid: Uuid,
        env: &str,
        new_status: NodeStatus,
    ) -> Result<(), Box<dyn Error>> {
        let client = self.client.lock().await;

        let query = "
            UPDATE nodes
            SET status = $1, modified_at = $2
            WHERE uuid = $3 AND env = $4
        ";

        let status_str = new_status.to_string();
        let modified_at = Utc::now();

        let result = client
            .execute(query, &[&status_str, &modified_at, &uuid, &env])
            .await;

        match result {
            Ok(rows_updated) => {
                if rows_updated == 0 {
                    warn!("No node found with UUID {}", uuid);
                } else {
                    debug!("Updated node {} status to {}", uuid, status_str);
                }
            }
            Err(e) => {
                error!("Error updating node status: {}", e);
                return Err(e.into());
            }
        }

        Ok(())
    }
}
