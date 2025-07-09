use async_trait::async_trait;
use pony::Proto;

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

        let topic0 = self.subscriber.topics[0].clone();
        let topic1 = self.subscriber.topics[1].clone();
        let _topic2 = self.subscriber.topics[2].clone();

        assert!(self.subscriber.topics.contains(&"all".to_string()));

        loop {
            if let Some(data) = sub.recv().await {
                let mut parts = data.splitn(2, ' ');
                let topic_str = parts.next().unwrap_or("");
                let payload = parts.next().unwrap_or("");

                match Topic::from_raw(topic_str) {
                    Topic::Init(uuid) if uuid != topic0 => {
                        log::warn!("SUB: Skipping init for another node: {}", uuid);
                        continue;
                    }
                    Topic::Updates(env) if env != topic1 => {
                        log::warn!("SUB: Skipping update for another env: {}", env);
                        continue;
                    }
                    Topic::Unknown(raw) => {
                        log::warn!("SUB: Unknown topic: {}", raw);
                        continue;
                    }
                    Topic::All => {
                        log::debug!("SUB: message for All topic recieved");
                    }
                    _ => {}
                }

                match serde_json::from_str::<Message>(payload) {
                    Ok(message) => {
                        if let Err(err) = self.handle_message(message).await {
                            log::error!("ZMQ SUB: Failed to handle message: {}", err);
                        }
                    }
                    Err(err) => {
                        log::error!(
                            "ZMQ SUB: Failed to parse payload: {}\nError: {}",
                            payload,
                            err
                        );
                    }
                }
            }
        }
    }

    async fn handle_message(&self, msg: Message) -> Result<()> {
        match msg.action {
            Action::Create | Action::Update => {
                let conn_id = msg.conn_id;

                match msg.tag {
                    Tag::Wireguard => {
                        let wg = msg
                            .wg
                            .clone()
                            .ok_or_else(|| PonyError::Custom("Missing WireGuard keys".into()))?;

                        let node_id = {
                            let mem = self.memory.lock().await;
                            let node = mem.nodes.get_self();
                            node.map(|n| n.uuid).ok_or_else(|| {
                                PonyError::Custom("Current node UUID not found".to_string())
                            })?
                        };
                        let proto = Proto::new_wg(&wg, &node_id);

                        let conn = Connection::new(proto);

                        {
                            let mut mem = self.memory.lock().await;
                            mem.connections
                                .add(&conn_id, conn.clone().into())
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

                        wg_api.create(&wg.keys.pubkey, wg.address).map_err(|e| {
                            PonyError::Custom(format!("Failed to create WireGuard peer: {}", e))
                        })?;

                        log::debug!("Created {}", conn);

                        Ok(())
                    }

                    Tag::VlessXtls | Tag::VlessGrpc | Tag::Vmess => {
                        let proto = Proto::new_xray(&msg.tag);
                        let conn = Connection::new(proto);

                        let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                            PonyError::Grpc(Status::unavailable("Xray handler unavailable"))
                        })?;

                        client
                            .create(&msg.conn_id, msg.tag, None)
                            .await
                            .map_err(|err| {
                                PonyError::Custom(format!(
                                    "Failed to create conn {}: {}",
                                    msg.conn_id, err
                                ))
                            })?;

                        let mut mem = self.memory.lock().await;
                        mem.connections.add(&conn_id, conn.into()).map_err(|err| {
                            PonyError::Custom(format!("Failed to add conn {}: {}", conn_id, err))
                        })?;

                        Ok(())
                    }
                    Tag::Shadowsocks => {
                        if let Some(password) = msg.password {
                            let proto = Proto::new_ss(&password);
                            let conn = Connection::new(proto);

                            let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                                PonyError::Grpc(Status::unavailable("Xray handler unavailable"))
                            })?;

                            client
                                .create(&msg.conn_id, msg.tag, Some(password))
                                .await
                                .map_err(|err| {
                                    PonyError::Custom(format!(
                                        "Failed to create conn {}: {}",
                                        msg.conn_id, err
                                    ))
                                })?;

                            let mut mem = self.memory.lock().await;
                            mem.connections.add(&conn_id, conn.into()).map_err(|err| {
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

            Action::Delete => match msg.tag {
                Tag::Wireguard => {
                    let wg_api = self
                        .wg_client
                        .as_ref()
                        .ok_or_else(|| PonyError::Custom("WireGuard API is unavailable".into()))?;

                    let wg = msg.wg.clone().ok_or_else(|| {
                        PonyError::Custom("Missing WireGuard keys in message".into())
                    })?;

                    wg_api.delete(&wg.keys.pubkey).map_err(|e| {
                        PonyError::Custom(format!("Failed to delete WireGuard peer: {}", e))
                    })?;

                    let mut mem = self.memory.lock().await;
                    let _ = mem.connections.remove(&msg.conn_id);

                    Ok(())
                }
                Tag::VlessXtls | Tag::VlessGrpc | Tag::Vmess | Tag::Shadowsocks => {
                    let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                        PonyError::Grpc(Status::unavailable("Xray handler unavailable"))
                    })?;

                    client
                        .remove(&msg.conn_id, msg.tag, msg.password)
                        .await
                        .map_err(|e| {
                            PonyError::Custom(format!(
                                "Couldn't remove connection from Xray: {}",
                                e
                            ))
                        })?;

                    let mut mem = self.memory.lock().await;
                    let _ = mem.connections.remove(&msg.conn_id);

                    Ok(())
                }
            },

            Action::ResetStat => {
                self.reset_stat(&msg.conn_id)
                    .await
                    .map_err(|e| PonyError::Custom(format!("Couldn't reset stat: {}", e)))?;
                log::debug!("Reset stat for {}", msg.conn_id);
                Ok(())
            }
        }
    }
}
