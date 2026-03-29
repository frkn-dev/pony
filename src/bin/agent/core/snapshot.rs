use rkyv::Archive;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::memory::connection::wireguard::IpAddrMaskSerializable;
use pony::memory::snapshot::SnapshotManager;
use pony::wireguard_op::WgApi;
use pony::xray_op::client::HandlerActions;
use pony::xray_op::client::HandlerClient;
use pony::Connection;
use pony::ConnectionBaseOp as ConnOp;
use pony::NodeStorageOp as NodeOp;
use pony::Result;
use pony::Tag;

#[async_trait::async_trait]
pub trait SnapshotRestore {
    async fn restore_connections(
        &self,
        xray_client: Option<Arc<Mutex<HandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Result<()>;
}

#[async_trait::async_trait]
impl<N, C, S> SnapshotRestore for SnapshotManager<N, C, S>
where
    C: Archive + Send + Sync + Clone + 'static + ConnOp + From<Connection>,
    N: Send + Sync + Clone + 'static + NodeOp,
    S: Send + Sync + Clone + 'static,
{
    async fn restore_connections(
        &self,
        xray_client: Option<Arc<Mutex<HandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Result<()> {
        let mem = self.memory.read().await;

        for (conn_id, conn) in mem.connections.iter() {
            let conn_id = *conn_id;
            let conn = conn.clone();
            let memory = self.memory.clone();
            let wg_client = wg_client.clone();
            let xray_client = xray_client.clone();

            tokio::spawn(async move {
                match conn.get_proto().proto() {
                    Tag::Wireguard => {
                        if let Some(wg) = conn.get_wireguard() {
                            let node_id = {
                                let mem = memory.read().await;
                                mem.nodes.get_self().map(|n| n.uuid)
                            };

                            if let Some(_node_id) = node_id {
                                if let Some(api) = wg_client.as_ref() {
                                    if let Err(e) = api.create(
                                        &wg.keys.pubkey,
                                        <IpAddrMaskSerializable as Clone>::clone(&wg.address)
                                            .into(),
                                    ) {
                                        log::error!(
                                            "Failed to restore WireGuard connection {}: {}",
                                            conn_id,
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Tag::VlessTcpReality
                    | Tag::VlessGrpcReality
                    | Tag::VlessXhttpReality
                    | Tag::Vmess => {
                        if let Some(client) = xray_client.as_ref() {
                            if let Err(e) = client
                                .create(&conn_id, conn.get_proto().proto(), None)
                                .await
                            {
                                log::error!("Failed to restore Xray connection {}: {}", conn_id, e);
                            } else {
                                log::debug!("Restored Xray connection {}", conn_id);
                            }
                        }
                    }
                    Tag::Shadowsocks => {
                        if let Some(password) = conn.get_password() {
                            if let Some(client) = xray_client.as_ref() {
                                if let Err(e) = client
                                    .create(&conn_id, conn.get_proto().proto(), Some(password))
                                    .await
                                {
                                    log::error!(
                                        "Failed to restore Shadowsocks connection {}: {}",
                                        conn_id,
                                        e
                                    );
                                } else {
                                    log::debug!("Restored Shadowsocks connection {}", conn_id);
                                }
                            }
                        }
                    }
                    Tag::Hysteria2 | Tag::Mtproto => {
                        log::warn!(
                            "Skipping unsupported connection {} with tag {:?}",
                            conn_id,
                            conn.get_proto().proto()
                        );
                    }
                }
            });
        }

        Ok(())
    }
}
