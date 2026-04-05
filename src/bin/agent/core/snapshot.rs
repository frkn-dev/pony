use rkyv::Archive;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::memory::connection::wireguard::IpAddrMaskSerializable;
use pony::memory::connection::Connections;
use pony::memory::snapshot::SnapshotManager;
use pony::wireguard_op::WgApi;
use pony::xray_op::client::HandlerActions;
use pony::xray_op::client::HandlerClient;
use pony::ConnectionBaseOp as ConnOp;
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
impl<C> SnapshotRestore for SnapshotManager<Connections<C>>
where
    C: Archive + Send + Sync + Clone + 'static + ConnOp,
{
    async fn restore_connections(
        &self,
        xray_client: Option<Arc<Mutex<HandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Result<()> {
        let mem = self.memory.read().await;

        if mem.is_empty() {
            return Err(pony::PonyError::Custom("Empty snapshot".into()));
        }

        let conns: Vec<(uuid::Uuid, C)> =
            mem.iter().map(|(id, conn)| (*id, conn.clone())).collect();

        drop(mem);

        for (conn_id, conn) in conns {
            let wg_client = wg_client.clone();
            let xray_client = xray_client.clone();

            tokio::spawn(async move {
                match conn.get_proto().proto() {
                    Tag::Wireguard => {
                        if let Some(wg) = conn.get_wireguard() {
                            if let Some(api) = wg_client.as_ref() {
                                if let Err(e) = api.create(
                                    &wg.keys.pubkey,
                                    <IpAddrMaskSerializable as Clone>::clone(&wg.address).into(),
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
