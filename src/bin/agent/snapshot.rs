use rkyv::Archive;
use std::sync::Arc;
use tokio::sync::Mutex;

use pony::{
    ConnectionBaseOperations, Connections, Error, Result, SnapshotManager, Tag, WgApi,
    XrayHandlerActions, XrayHandlerClient,
};

#[async_trait::async_trait]
pub trait SnapshotRestore {
    async fn restore_connections(
        &self,
        xray_client: Option<Arc<Mutex<XrayHandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Result<()>;
}

#[async_trait::async_trait]
impl<C> SnapshotRestore for SnapshotManager<Connections<C>>
where
    C: Archive + Send + Sync + Clone + 'static + ConnectionBaseOperations,
{
    async fn restore_connections(
        &self,
        xray_client: Option<Arc<Mutex<XrayHandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Result<()> {
        let mem = self.memory.read().await;

        if mem.is_empty() {
            return Err(Error::Custom("Empty snapshot".into()));
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
                                if let Ok(pubkey) = &wg.keys.pubkey() {
                                    if let Err(e) = api.create(pubkey, wg.address.clone()) {
                                        tracing::error!(
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
                                tracing::error!(
                                    "Failed to restore Xray connection {}: {}",
                                    conn_id,
                                    e
                                );
                            } else {
                                tracing::debug!("Restored Xray connection {}", conn_id);
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
                                    tracing::error!(
                                        "Failed to restore Shadowsocks connection {}: {}",
                                        conn_id,
                                        e
                                    );
                                } else {
                                    tracing::debug!("Restored Shadowsocks connection {}", conn_id);
                                }
                            }
                        }
                    }
                    Tag::Hysteria2 | Tag::Mtproto => {
                        tracing::warn!(
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
