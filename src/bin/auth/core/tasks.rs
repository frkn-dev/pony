use async_trait::async_trait;
use pony::ConnectionStorageBaseOp;
use pony::Proto;
use rkyv::AlignedVec;
use rkyv::Infallible;
use tokio::time::Duration;

use pony::Action;
use pony::BaseConnection as Connection;
use pony::ConnectionBaseOp;
use pony::Message;
use pony::NodeStorageOp;
use pony::SubscriptionOp;
use pony::Topic;
use pony::{PonyError, Result};

use rkyv::Deserialize;

use super::AuthService;

#[async_trait]
pub trait Tasks {
    async fn run_subscriber(&self) -> Result<()>;
    async fn handle_messages_batch(&self, msg: Vec<Message>) -> Result<()>;
}

#[async_trait]
impl<T, C, S> Tasks for AuthService<T, C, S>
where
    T: NodeStorageOp + Send + Sync + Clone,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static + From<Connection>,
    S: SubscriptionOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
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

            if payload_bytes.is_empty() {
                log::warn!("SUB: Empty payload, skipping");
                continue;
            }

            let mut aligned = AlignedVec::new();
            aligned.extend_from_slice(&payload_bytes);

            let archived = match { rkyv::check_archived_root::<Vec<Message>>(&aligned) } {
                Ok(a) => a,
                Err(e) => {
                    log::error!("SUB: Invalid rkyv root: {:?}", e);
                    log::error!("SUB: Payload bytes (hex) = {}", hex::encode(payload_bytes));
                    continue;
                }
            };

            match archived.deserialize(&mut Infallible) {
                Ok(messages) => {
                    if let Err(err) = self.handle_messages_batch(messages).await {
                        log::error!("SUB: Failed to handle messages: {}", err);
                    }
                }
                Err(err) => {
                    log::error!("SUB: Failed to deserialize messages: {}", err);
                    log::error!("SUB: Payload bytes (hex) = {}", hex::encode(payload_bytes));
                }
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn handle_messages_batch(&self, messages: Vec<Message>) -> Result<()> {
        let mut mem = self.memory.write().await;

        log::debug!("Got {} messages", messages.len());

        for msg in messages {
            let conn_id: uuid::Uuid = msg.conn_id.into();

            let res: Result<()> = match msg.action {
                Action::Create | Action::Update => {
                    if let Some(token) = msg.token {
                        let proto = Proto::new_hysteria2(&token);
                        let conn = Connection::new(
                            proto,
                            msg.expires_at.map(Into::into),
                            msg.subscription_id,
                        );
                        mem.connections
                            .add(&conn_id, conn.into())
                            .map(|_| ())
                            .map_err(|err| {
                                PonyError::Custom(format!(
                                    "Failed to add conn {}: {}",
                                    conn_id, err
                                ))
                            })
                    } else {
                        log::debug!("Skipped message {:?}", msg);
                        Ok(())
                    }
                }

                Action::Delete => {
                    let _ = mem.connections.remove(&conn_id);
                    Ok(())
                }

                _ => Ok(()),
            };

            res?;
        }

        Ok(())
    }
}
