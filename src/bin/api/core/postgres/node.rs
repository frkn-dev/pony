use defguard_wireguard_rs::net::IpAddrMask;

use std::collections::HashMap;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use tokio::sync::Mutex;

use pony::config::wireguard::WireguardSettings;
use pony::config::xray::Inbound;
use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::utils::to_ipv4;
use pony::Result;

use super::PgClientManager;

pub struct PgNode {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgNode {
    pub fn new(manager: Arc<Mutex<PgClientManager>>) -> Self {
        Self { manager }
    }

    pub async fn upsert(&self, node_id: uuid::Uuid, node: Node) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;
        let cores = node.cores as i32;

        let address: IpAddr = IpAddr::V4(node.address);

        let node_query = "
        INSERT INTO nodes (
            id, uuid, env, hostname, address, status, created_at, modified_at, label, interface, cores, max_bandwidth_bps
        )
        VALUES (
            $1, $2, $3, $4, $5, $6::node_status, $7, $8, $9, $10, $11, $12
        )
        ON CONFLICT (id) DO UPDATE SET
            uuid = EXCLUDED.uuid,
            env = EXCLUDED.env,
            hostname = EXCLUDED.hostname,
            address = EXCLUDED.address,
            status = EXCLUDED.status,
            modified_at = EXCLUDED.modified_at,
            label = EXCLUDED.label,
            interface = EXCLUDED.interface,
            cores = EXCLUDED.cores,
            max_bandwidth_bps = EXCLUDED.max_bandwidth_bps
    ";

        tx.execute(
            node_query,
            &[
                &node_id,
                &node.uuid,
                &node.env,
                &node.hostname,
                &address,
                &node.status,
                &node.created_at,
                &node.modified_at,
                &node.label,
                &node.interface,
                &cores,
                &node.max_bandwidth_bps,
            ],
        )
        .await?;

        let inbound_query = "
        INSERT INTO inbounds (
            id, node_id, tag, port, stream_settings,
            uplink, downlink, conn_count,
            wg_pubkey, wg_privkey, wg_interface, wg_network, wg_address, dns
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8,
            $9, $10, $11, $12, $13, $14
        )
        ON CONFLICT (node_id, tag) DO UPDATE SET
            port = EXCLUDED.port,
            stream_settings = EXCLUDED.stream_settings,
            uplink = EXCLUDED.uplink,
            downlink = EXCLUDED.downlink,
            conn_count = EXCLUDED.conn_count,
            wg_pubkey = EXCLUDED.wg_pubkey,
            wg_privkey = EXCLUDED.wg_privkey,
            wg_interface = EXCLUDED.wg_interface,
            wg_network = EXCLUDED.wg_network,
            wg_address = EXCLUDED.wg_address,
            dns = EXCLUDED.dns
    ";

        for inbound in node.inbounds.values() {
            let inbound_id = uuid::Uuid::new_v4();
            let stream_settings = serde_json::to_value(&inbound.stream_settings)?;

            let (wg_pubkey, wg_privkey, wg_interface, wg_network, wg_address, dns) = inbound
                .wg
                .as_ref()
                .map(|wg| {
                    (
                        Some(&wg.pubkey),
                        Some(&wg.privkey),
                        Some(&wg.interface),
                        Some(wg.network.to_string()),
                        Some(wg.address.to_string()),
                        Some(
                            wg.dns
                                .iter()
                                .cloned()
                                .map(IpAddr::V4)
                                .collect::<Vec<IpAddr>>(),
                        ),
                    )
                })
                .unwrap_or((None, None, None, None, None, None));

            tx.execute(
                inbound_query,
                &[
                    &inbound_id,
                    &node_id,
                    &inbound.tag,
                    &(inbound.port as i32),
                    &stream_settings,
                    &inbound.uplink,
                    &inbound.downlink,
                    &inbound.conn_count,
                    &wg_pubkey,
                    &wg_privkey,
                    &wg_interface,
                    &wg_network,
                    &wg_address,
                    &dns,
                ],
            )
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn insert(&self, node_id: uuid::Uuid, node: Node) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;
        let cores = node.cores as i32;

        let node_query = "
        INSERT INTO nodes (id, uuid, env, hostname, address, status, created_at, modified_at, label, interface, cores, max_bandwidth_bps)
        VALUES ($1, $2, $3, $4, $5, $6::node_status, $7, $8, $9, $10)
    ";
        let address: IpAddr = IpAddr::V4(node.address);

        tx.execute(
            node_query,
            &[
                &node_id,
                &node.uuid,
                &node.env,
                &node.hostname,
                &address,
                &node.status,
                &node.created_at,
                &node.modified_at,
                &node.label,
                &node.interface,
                &cores,
                &node.max_bandwidth_bps,
            ],
        )
        .await?;

        let inbound_query = "
        INSERT INTO inbounds (
            id, node_id, tag, port, stream_settings,
            uplink, downlink, conn_count,
            wg_pubkey, wg_privkey, wg_interface, wg_network, wg_address, dns
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8,
            $9, $10, $11, $12, $13, $14
        )
    ";

        for inbound in node.inbounds.values() {
            let inbound_id = uuid::Uuid::new_v4();
            let stream_settings = serde_json::to_value(&inbound.stream_settings)?;

            let (wg_pubkey, wg_privkey, wg_interface, wg_network, wg_address, dns) = inbound
                .wg
                .as_ref()
                .map(|wg| {
                    (
                        Some(&wg.pubkey),
                        Some(&wg.privkey),
                        Some(&wg.interface),
                        Some(wg.network.to_string()),
                        Some(wg.address.to_string()),
                        Some(
                            wg.dns
                                .iter()
                                .cloned()
                                .map(IpAddr::V4)
                                .collect::<Vec<IpAddr>>(),
                        ),
                    )
                })
                .unwrap_or((None, None, None, None, None, None));

            tx.execute(
                inbound_query,
                &[
                    &inbound_id,
                    &node_id,
                    &inbound.tag,
                    &(inbound.port as i32),
                    &stream_settings,
                    &inbound.uplink,
                    &inbound.downlink,
                    &inbound.conn_count,
                    &wg_pubkey,
                    &wg_privkey,
                    &wg_interface,
                    &wg_network,
                    &wg_address,
                    &dns,
                ],
            )
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    pub async fn all(&self) -> Result<Vec<Node>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                "SELECT
                n.id AS node_id, n.uuid, n.env, n.hostname, n.address, n.status,
                n.created_at, n.modified_at, n.label, n.interface, n.cores, n.max_bandwidth_bps,
                i.id AS inbound_id, i.tag, i.port, i.stream_settings, i.uplink, i.downlink,
                i.conn_count, i.wg_pubkey, i.wg_privkey, i.wg_interface, i.wg_network, i.wg_address, i.dns
             FROM nodes n
             LEFT JOIN inbounds i ON n.id = i.node_id",
                &[],
            )
            .await?;

        let mut nodes_map: HashMap<uuid::Uuid, Node> = HashMap::new();

        for row in rows {
            let node_id: uuid::Uuid = row.get("node_id");
            let uuid: uuid::Uuid = row.get("uuid");
            let env: String = row.get("env");
            let hostname: String = row.get("hostname");
            let address: IpAddr = row.get("address");
            let status: NodeStatus = row.get("status");
            let created_at: DateTime<Utc> = row.get("created_at");
            let modified_at: DateTime<Utc> = row.get("modified_at");
            let label: String = row.get("label");
            let interface: String = row.get("interface");
            let cores: i32 = row.get("cores");
            let max_bandwidth_bps: i64 = row.get("max_bandwidth_bps");

            let wg_network: Option<IpAddrMask> = row
                .get::<_, Option<String>>("wg_network")
                .and_then(|s| s.parse().ok());

            let wg_address: Option<Ipv4Addr> = row
                .get::<_, Option<String>>("wg_address")
                .and_then(|s| s.parse().ok());

            let dns: Option<Vec<Ipv4Addr>> = row.get::<_, Option<Vec<IpAddr>>>("dns").map(|ips| {
                ips.into_iter()
                    .filter_map(|ip| match ip {
                        IpAddr::V4(v4) => Some(v4),
                        _ => None,
                    })
                    .collect()
            });
            let inbound_id: Option<uuid::Uuid> = row.get("inbound_id");

            if let Some(ipv4_addr) = to_ipv4(address) {
                let node_entry = nodes_map.entry(node_id).or_insert_with(|| Node {
                    uuid,
                    env: env.clone(),
                    hostname: hostname.clone(),
                    address: ipv4_addr,
                    interface: interface.clone(),
                    status,
                    created_at,
                    modified_at,
                    label: label.clone(),
                    inbounds: HashMap::new(),
                    cores: cores as usize,
                    max_bandwidth_bps: max_bandwidth_bps,
                });

                if let Some(_inbound_id) = inbound_id {
                    let wg = match (
                        row.get::<_, Option<String>>("wg_pubkey"),
                        row.get::<_, Option<String>>("wg_privkey"),
                        row.get::<_, Option<String>>("wg_interface"),
                        wg_network.clone(),
                        wg_address,
                        dns,
                    ) {
                        (
                            Some(pubkey),
                            Some(privkey),
                            Some(interface),
                            Some(network),
                            Some(address),
                            Some(dns),
                        ) => Some(WireguardSettings {
                            pubkey,
                            privkey,
                            interface,
                            network,
                            address,
                            port: row.get::<_, i32>("port") as u16,
                            dns: dns,
                        }),
                        _ => None,
                    };

                    let inbound = Inbound {
                        tag: row.get("tag"),
                        port: row.get::<_, i32>("port") as u16,
                        stream_settings: row
                            .get::<_, Option<serde_json::Value>>("stream_settings")
                            .map(|v| serde_json::from_value(v).ok())
                            .flatten(),

                        uplink: row.get("uplink"),
                        downlink: row.get("downlink"),
                        conn_count: row.get("conn_count"),
                        wg,
                    };

                    node_entry.inbounds.insert(inbound.tag, inbound);
                }
            }
        }

        Ok(nodes_map.into_values().collect())
    }

    pub async fn update_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        new_status: NodeStatus,
    ) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

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
