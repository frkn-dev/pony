use async_trait::async_trait;
use defguard_wireguard_rs::net::IpAddrMask;
use futures::future::try_join_all;
use rkyv::AlignedVec;
use rkyv::Deserialize;
use rkyv::Infallible;
use tokio::time::Duration;
use tonic::Status;

use pony::{
    Action, BaseConnection as Connection, ConnectionBaseOperations,
    ConnectionStorageBaseOperations, Message, Metrics, Proto, StatsOp, Tag, Topic,
    XrayHandlerActions,
};
use pony::{Error, Result};

use super::agent::Agent;
use super::metrics::BusinessMetrics;

#[async_trait]
pub trait Tasks {
    async fn run_subscriber(&self) -> Result<()>;
    async fn handle_messages_batch(&self, msg: Vec<Message>) -> Result<()>;
    async fn handle_message(&self, msg: Message) -> Result<()>;
    async fn collect_metrics(&self);
}

#[async_trait]
impl<C> Tasks for Agent<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static + From<Connection>,
{
    async fn run_subscriber(&self) -> Result<()> {
        let sub = self.subscriber.clone();
        assert!(self.subscriber.topics.contains(&"all".to_string()));

        loop {
            let Some((topic_bytes, payload_bytes)) = sub.recv().await else {
                tracing::warn!("SUB: No multipart message received");
                continue;
            };

            let topic_str = std::str::from_utf8(&topic_bytes).unwrap_or("<invalid utf8>");
            tracing::debug!("SUB: Topic string: {:?}", topic_str);
            tracing::debug!("SUB: Payload {} bytes", payload_bytes.len());

            match Topic::from_raw(topic_str) {
                Topic::Init(uuid) if uuid != self.subscriber.topics[0] => {
                    tracing::warn!("SUB: Skipping init for another node: {}", uuid);
                    continue;
                }
                Topic::Updates(env) if env != self.subscriber.topics[1] => {
                    tracing::warn!("SUB: Skipping update for another env: {}", env);
                    continue;
                }
                Topic::Unknown(raw) => {
                    tracing::warn!("SUB: Unknown topic: {}", raw);
                    continue;
                }
                Topic::All => {
                    tracing::debug!("SUB: Message for 'All' topic received");
                }
                topic => {
                    tracing::debug!("SUB: Accepted topic: {:?}", topic);
                }
            }

            if payload_bytes.is_empty() {
                tracing::warn!("SUB: Empty payload, skipping");
                continue;
            }

            let mut aligned = AlignedVec::new();
            aligned.extend_from_slice(&payload_bytes);

            let archived = match rkyv::check_archived_root::<Vec<Message>>(&aligned) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("SUB: Invalid rkyv root: {:?}", e);
                    tracing::error!("SUB: Payload bytes (hex) = {}", hex::encode(payload_bytes));
                    continue;
                }
            };

            match archived.deserialize(&mut Infallible) {
                Ok(messages) => {
                    if let Err(err) = self.handle_messages_batch(messages).await {
                        tracing::error!("SUB: Failed to handle messages: {}", err);
                    }
                }
                Err(err) => {
                    tracing::error!("SUB: Failed to deserialize messages: {}", err);
                    tracing::error!("SUB: Payload bytes (hex) = {}", hex::encode(payload_bytes));
                }
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn handle_messages_batch(&self, messages: Vec<Message>) -> Result<()> {
        tracing::debug!("Got {} messages", messages.len());

        let handles: Vec<_> = messages
            .into_iter()
            .map(|msg| self.handle_message(msg))
            .collect();

        let _results = try_join_all(handles).await?;

        Ok(())
    }

    async fn handle_message(&self, msg: Message) -> Result<()> {
        match msg.action {
            Action::Create | Action::Update => {
                let conn_id: uuid::Uuid = msg.conn_id;

                match msg.tag {
                    Tag::Wireguard => {
                        let wg = msg
                            .wg
                            .clone()
                            .ok_or_else(|| Error::Custom("Missing WireGuard keys".into()))?;

                        let node_id = self.node.uuid;
                        let proto = Proto::new_wg(&wg, &node_id);

                        let conn = Connection::new(
                            proto,
                            msg.expires_at.map(Into::into),
                            msg.subscription_id,
                        );

                        {
                            let mut mem = self.memory.write().await;
                            mem.add(&conn_id.clone(), conn.clone().into())
                                .map_err(|err| {
                                    Error::Custom(format!(
                                        "Failed to add WireGuard conn {}: {}",
                                        conn_id, err
                                    ))
                                })?;
                        }

                        let wg_api = self
                            .wg_client
                            .as_ref()
                            .ok_or_else(|| Error::Custom("WireGuard API is unavailable".into()))?;

                        let exist = wg_api.is_exist(wg.keys.pubkey.clone());

                        if exist {
                            return Err(Error::Custom("WG User already exist".into()));
                        }

                        let wg_address: IpAddrMask = wg.address.into();

                        wg_api.create(&wg.keys.pubkey, wg_address).map_err(|e| {
                            Error::Custom(format!("Failed to create WireGuard peer: {}", e))
                        })?;

                        tracing::debug!("Connection Created {}", conn);

                        return Ok(());
                    }

                    Tag::VlessTcpReality
                    | Tag::VlessGrpcReality
                    | Tag::VlessXhttpReality
                    | Tag::Vmess => {
                        let proto = Proto::new_xray(&msg.tag);
                        let conn = Connection::new(
                            proto,
                            msg.expires_at.map(Into::into),
                            msg.subscription_id,
                        );

                        let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                            Error::Grpc(Box::new(Status::unavailable("Xray handler unavailable")))
                        })?;

                        client
                            .create(&conn_id.clone(), msg.tag, None)
                            .await
                            .map_err(|err| {
                                Error::Custom(format!(
                                    "Failed to create conn {}: {}",
                                    conn_id.clone(),
                                    err
                                ))
                            })?;

                        let mut mem = self.memory.write().await;
                        mem.add(&conn_id.clone(), conn.into()).map_err(|err| {
                            Error::Custom(format!("Failed to add conn {}: {}", conn_id, err))
                        })?;

                        return Ok(());
                    }
                    Tag::Shadowsocks => {
                        if let Some(password) = msg.password {
                            let proto = Proto::new_ss(&password);
                            let conn = Connection::new(
                                proto,
                                msg.expires_at.map(Into::into),
                                msg.subscription_id,
                            );

                            let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                                Error::Grpc(Box::new(Status::unavailable(
                                    "Xray handler unavailable",
                                )))
                            })?;

                            client
                                .create(&conn_id.clone(), msg.tag, Some(password))
                                .await
                                .map_err(|err| {
                                    Error::Custom(format!(
                                        "Failed to create conn {}: {}",
                                        conn_id, err
                                    ))
                                })?;

                            let mut mem = self.memory.write().await;
                            mem.add(&conn_id.clone(), conn.into()).map_err(|err| {
                                Error::Custom(format!("Failed to add conn {}: {}", conn_id, err))
                            })?;

                            return Ok(());
                        } else {
                            return Err(Error::Custom(
                                "Password not provided for Shadowsocks user".into(),
                            ));
                        }
                    }
                    Tag::Hysteria2 => {
                        return Err(Error::Custom("Hysteria2 is not supported".into()))
                    }
                    Tag::Mtproto => return Err(Error::Custom("Mtproto is not supported".into())),
                }
            }

            Action::Delete => {
                let tag = msg.tag;
                let conn_id = msg.conn_id;
                match tag {
                    Tag::Wireguard => {
                        let wg_api = self
                            .wg_client
                            .as_ref()
                            .ok_or_else(|| Error::Custom("WireGuard API is unavailable".into()))?;

                        let wg = msg.wg.clone().ok_or_else(|| {
                            Error::Custom("Missing WireGuard keys in message".into())
                        })?;

                        wg_api.delete(&wg.keys.pubkey).map_err(|e| {
                            Error::Custom(format!("Failed to delete WireGuard peer: {}", e))
                        })?;

                        let mut mem = self.memory.write().await;
                        let _ = mem.remove(&conn_id);

                        return Ok(());
                    }
                    Tag::VlessTcpReality
                    | Tag::VlessGrpcReality
                    | Tag::VlessXhttpReality
                    | Tag::Vmess
                    | Tag::Shadowsocks => {
                        let client = self.xray_handler_client.as_ref().ok_or_else(|| {
                            Error::Grpc(Box::new(Status::unavailable("Xray handler unavailable")))
                        })?;

                        client
                            .remove(&conn_id.clone(), tag, msg.password)
                            .await
                            .map_err(|e| {
                                Error::Custom(format!(
                                    "Couldn't remove connection from Xray: {}",
                                    e
                                ))
                            })?;

                        let mut mem = self.memory.write().await;
                        let _ = mem.remove(&conn_id);

                        return Ok(());
                    }
                    Tag::Hysteria2 => {
                        return Err(Error::Custom("Hysteria2 is not supported".into()))
                    }
                    Tag::Mtproto => return Err(Error::Custom("Mtproto is not supported".into())),
                }
            }

            Action::ResetStat => {
                self.reset(&msg.conn_id)
                    .await
                    .map_err(|e| Error::Custom(format!("Couldn't reset stat: {}", e)))?;
                tracing::debug!("Reset stat for {}", msg.conn_id);
                return Ok(());
            }
        }
    }

    async fn collect_metrics(&self) {
        self.heartbeat().await;
        self.bandwidth().await;
        self.cpu_usage().await;
        self.loadavg().await;
        self.memory().await;
        self.disk_usage().await;

        if self.xray_stats_client.is_some() {
            self.collect_inbound_metrics().await;
            self.collect_user_metrics().await;
        }

        if self.wg_client.is_some() {
            self.collect_wg_metrics().await;
        }
    }
}
