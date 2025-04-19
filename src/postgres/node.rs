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

pub struct PgNodeRequest {
    pub client: Arc<Mutex<Client>>,
}

impl PgNodeRequest {
    pub fn new(client: Arc<Mutex<Client>>) -> Self {
        Self { client }
    }

    pub async fn insert_node(&self, node: Node) -> Result<(), Box<dyn Error>> {
        let client = self.client.lock().await;

        let query = "
            INSERT INTO nodes (hostname, ipv4, status, inbounds, env, uuid, created_at, modified_at, label, iface)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ";

        let status_str = node.status.to_string();
        let ip: IpAddr = IpAddr::V4(node.ipv4);
        let inbounds_json = serde_json::to_value(&node.inbounds)?;

        let result = client
            .execute(
                query,
                &[
                    &node.hostname,
                    &ip,
                    &status_str,
                    &inbounds_json,
                    &node.env,
                    &node.uuid,
                    &node.created_at,
                    &node.modified_at,
                    &node.label,
                    &node.iface,
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
            "SELECT hostname, ipv4, status, inbounds, env, uuid, created_at, modified_at, label, iface 
             FROM nodes",
            &[],
        )
        .await?;

        let nodes: Vec<Node> = rows
            .into_iter()
            .filter_map(|row| {
                let hostname: String = row.get(0);
                let ipv4: IpAddr = row.get(1);
                let status: String = row.get(2);
                let inbounds: Value = row.get(3);
                let env: String = row.get(4);
                let uuid: Uuid = row.get(5);
                let created_at: DateTime<Utc> = row.get(6);
                let modified_at: DateTime<Utc> = row.get(7);
                let label: String = row.get(8);
                let iface: String = row.get(9);

                if let Some(ipv4_addr) = to_ipv4(ipv4) {
                    Some(Node {
                        hostname,
                        ipv4: ipv4_addr,
                        status: NodeStatus::from_str(&status).unwrap_or(NodeStatus::Unknown),
                        inbounds: serde_json::from_value(inbounds).unwrap_or_default(),
                        env,
                        uuid,
                        created_at,
                        modified_at,
                        label: label,
                        iface: iface,
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
        new_status: NodeStatus,
    ) -> Result<(), Box<dyn Error>> {
        let client = self.client.lock().await;

        let query = "
            UPDATE nodes
            SET status = $1, modified_at = $2
            WHERE uuid = $3
        ";

        let status_str = new_status.to_string();
        let modified_at = Utc::now();

        let result = client
            .execute(query, &[&status_str, &modified_at, &uuid])
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
