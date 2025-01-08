use crate::state::node::NodeStatus;
use chrono::DateTime;
use chrono::Utc;
use serde_json::Value;
use std::error::Error;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::state::node::Node;

fn to_ipv4(ip: IpAddr) -> Option<Ipv4Addr> {
    match ip {
        IpAddr::V4(ipv4) => Some(ipv4),
        IpAddr::V6(_) => None,
    }
}

pub async fn nodes_db_request(client: Arc<Mutex<Client>>) -> Result<Vec<Node>, Box<dyn Error>> {
    let client = client.lock().await;

    let rows = client
        .query(
            "SELECT hostname, ipv4, status, inbounds, env, uuid, created_at, modified_at 
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
                })
            } else {
                None
            }
        })
        .collect();

    Ok(nodes)
}
