use async_trait::async_trait;
use defguard_wireguard_rs::net::IpAddrMask;
use pony::Proto;
use rkyv::AlignedVec;

use tonic::Status;

use pony::xray_op::client::HandlerActions;
use pony::xray_op::stats::StatOp;
use pony::Action;
use pony::BaseConnection as Connection;
use pony::ConnectionBaseOp;
use pony::ConnectionStorageBaseOp;
use pony::Message;
use pony::NodeStorageOp;
use pony::Tag;
use pony::Topic;
use pony::{PonyError, Result};
use rkyv::Deserialize;

use super::Agent;

#[async_trait]
pub trait Tasks {
    async fn run_subscriber(&self) -> Result<()>;
    async fn handle_message(&self, msg: Message) -> Result<()>;
}

#[async_trait]
impl<T, C> Tasks for Agent<T, C>
where
    T: NodeStorageOp + Send + Sync + Clone,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static + From<Connection>,
{
    async fn run_subscriber(&self) -> Result<()> {
        let sub = self.subscriber.clone();

        assert!(self.subscriber.topics.contains(&"all".to_string()));

        loop {
            let Some((topic_bytes, payload_bytes)) = sub.recv().await else {
                log::warn!("SUB: No multipart message received");
                continue;
            };

            let topic_str = std::str::from_utf8(&topic_bytes).unwrap_or("<invalid utf8>");
            log::debug!("SUB: Topic string: {:?}", topic_str);
            log::debug!("SUB: Payload {} bytes", payload_bytes.len());

            match Topic::from_raw(topic_str) {
                Topic::Init(uuid) if uuid != self.subscriber.topics[0] => {
                    log::warn!("SUB: Skipping init for another node: {}", uuid);
                    continue;
                }
                Topic::Updates(env) if env != self.subscriber.topics[1] => {
                    log::warn!("SUB: Skipping update for another env: {}", env);
                    continue;
                }
                Topic::Unknown(raw) => {
                    log::warn!("SUB: Unknown topic: {}", raw);
                    continue;
                }
                Topic::All => {
                    log::debug!("SUB: Message for 'All' topic received");
                }
                topic => {
                    log::debug!("SUB: Accepted topic: {:?}", topic);
                }
            }

            let mut aligned = AlignedVec::new();
            aligned.extend_from_slice(&payload_bytes);

            let archived = unsafe { rkyv::archived_root::<Message>(&aligned) };

            match archived.deserialize(&mut rkyv::Infallible) {
                Ok(message) => {
                    log::debug!("SUB: Successfully deserialized message: {}", message);
                    if let Err(err) = self.handle_message(message).await {
                        log::error!("ZMQ SUB: Failed to handle message: {}", err);
                    }
                }
                Err(err) => {
                    log::error!("ZMQ SUB: Failed to deserialize message: {}", err);
                    log::error!("SUB: Payload bytes (hex) = {}", hex::encode(payload_bytes));
                }
            }
        }
    }

    async fn handle_message(&self, msg: Message) -> Result<()> {
        match msg.action {
            Action::Create | Action::Update => {
                let conn_id: uuid::Uuid = msg.conn_id.clone().into();
                let tag = msg.tag;

                match tag {
                    Tag::Wireguard => {
                        let wg = msg
                            .wg
                            .clone()
                            .ok_or_else(|| PonyError::Custom("Missing WireGuard keys".into()))?;

                        let node_id = {
                            let mem = self.memory.read().await;
                            let node = mem.nodes.get_self();
                            node.map(|n| n.uuid).ok_or_else(|| {
                                PonyError::Custom("Current node UUID not found".to_string())
                            })?
                        };
                        let proto = Proto::new_wg(&wg, &node_id);

                        let conn = Connection::new(proto, None);

                        {
                            let mut mem = self.memory.write().await;
                            mem.connections
                                .add(&conn_id.clone(), conn.clone().into())
                                .map_err(|err| {
                                    PonyError::Custom(format!(
                                        "Failed to add WireGuard conn {}: {}",
                                        conn_id, err
                                    ))
                                })?;
                        }

                        let wg_api = self.wg_client.as_ref().ok_or_else(|| {
                            PonyError::Custom("WireGuard API is unavailable".into())
                        })?;

                        let exist = wg_api.is_exist(wg.keys.pubkey.clone());

                        if exist {
                            return Err(PonyError::Custom("WG User already exist".into()));
                        }

                        let wg_address: IpAddrMask = wg.address.into();

                        wg_api.create(&wg.keys.pubkey, wg_address).map_err(|e| {
                            PonyError::Custom(format!("Failed to create WireGuard peer: {}", e))
                        })?;

                        log::debug!("Created {}", conn);

                        Ok(())
                    }

                    Tag::VlessTcpReality
                    | Tag::VlessGrpcReality
                    | Tag::VlessXhttpReality
                    | Tag::Vmess => {
                        let proto = Proto::new_xray(&tag);
                        let conn = Connection::new(proto, None);

                        let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                            PonyError::Grpc(Status::unavailable("Xray handler unavailable"))
                        })?;

                        client
                            .create(&conn_id.clone(), tag, None)
                            .await
                            .map_err(|err| {
                                PonyError::Custom(format!(
                                    "Failed to create conn {}: {}",
                                    conn_id.clone(),
                                    err
                                ))
                            })?;

                        let mut mem = self.memory.write().await;
                        mem.connections
                            .add(&conn_id.clone(), conn.into())
                            .map_err(|err| {
                                PonyError::Custom(format!(
                                    "Failed to add conn {}: {}",
                                    conn_id, err
                                ))
                            })?;

                        Ok(())
                    }
                    Tag::Shadowsocks => {
                        if let Some(password) = msg.password {
                            let proto = Proto::new_ss(&password);
                            let conn = Connection::new(proto, None);

                            let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                                PonyError::Grpc(Status::unavailable("Xray handler unavailable"))
                            })?;

                            client
                                .create(&conn_id.clone(), tag, Some(password))
                                .await
                                .map_err(|err| {
                                    PonyError::Custom(format!(
                                        "Failed to create conn {}: {}",
                                        conn_id, err
                                    ))
                                })?;

                            let mut mem = self.memory.write().await;
                            mem.connections
                                .add(&conn_id.clone(), conn.into())
                                .map_err(|err| {
                                    PonyError::Custom(format!(
                                        "Failed to add conn {}: {}",
                                        conn_id, err
                                    ))
                                })?;

                            Ok(())
                        } else {
                            Err(PonyError::Custom(
                                "Password not provided for Shadowsocks user".into(),
                            ))
                        }
                    }
                }
            }

            Action::Delete => {
                let tag = msg.tag;
                let conn_id = msg.conn_id.clone().into();
                match tag {
                    Tag::Wireguard => {
                        let wg_api = self.wg_client.as_ref().ok_or_else(|| {
                            PonyError::Custom("WireGuard API is unavailable".into())
                        })?;

                        let wg = msg.wg.clone().ok_or_else(|| {
                            PonyError::Custom("Missing WireGuard keys in message".into())
                        })?;

                        wg_api.delete(&wg.keys.pubkey).map_err(|e| {
                            PonyError::Custom(format!("Failed to delete WireGuard peer: {}", e))
                        })?;

                        let mut mem = self.memory.write().await;
                        let _ = mem.connections.remove(&conn_id);

                        Ok(())
                    }
                    Tag::VlessTcpReality
                    | Tag::VlessGrpcReality
                    | Tag::VlessXhttpReality
                    | Tag::Vmess
                    | Tag::Shadowsocks => {
                        let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                            PonyError::Grpc(Status::unavailable("Xray handler unavailable"))
                        })?;

                        client
                            .remove(&conn_id.clone(), tag, msg.password)
                            .await
                            .map_err(|e| {
                                PonyError::Custom(format!(
                                    "Couldn't remove connection from Xray: {}",
                                    e
                                ))
                            })?;

                        let mut mem = self.memory.write().await;
                        let _ = mem.connections.remove(&conn_id);

                        Ok(())
                    }
                }
            }

            Action::ResetStat => {
                self.reset_stat(&msg.conn_id.clone().into())
                    .await
                    .map_err(|e| PonyError::Custom(format!("Couldn't reset stat: {}", e)))?;
                log::debug!("Reset stat for {}", msg.conn_id);
                Ok(())
            }
        }
    }
}
