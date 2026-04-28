use async_trait::async_trait;
use rkyv::AlignedVec;
use rkyv::Infallible;
use tokio::time::Duration;

use pony::{
    Action, BaseConnection as Connection, ConnectionBaseOperations,
    ConnectionStorageBaseOperations, Error, Message, Metrics, Proto, Result, Topic,
};

use rkyv::Deserialize;

use super::auth::AuthService;

#[async_trait]
pub trait Tasks {
    async fn run_subscriber(&self) -> Result<()>;
    async fn handle_messages_batch(&self, msg: Vec<Message>) -> Result<()>;
    async fn collect_metrics(&self);
}

#[async_trait]
impl<C> Tasks for AuthService<C>
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
        let mut mem = self.memory.write().await;

        tracing::debug!("Got {} messages", messages.len());

        for msg in messages {
            let conn_id: uuid::Uuid = msg.conn_id;

            let res: Result<()> = match msg.action {
                Action::Create | Action::Update => {
                    if let Some(token) = msg.token {
                        let proto = Proto::new_hysteria2(&token);
                        let conn = Connection::new(
                            proto,
                            msg.expires_at.map(Into::into),
                            msg.subscription_id,
                        );
                        mem.add(&conn_id, conn.into()).map(|_| ()).map_err(|err| {
                            Error::Custom(format!("Failed to add conn {}: {}", conn_id, err))
                        })
                    } else {
                        tracing::debug!("Skipped message {:?}", msg);
                        Ok(())
                    }
                }

                Action::Delete => {
                    let _ = mem.remove(&conn_id);
                    Ok(())
                }

                _ => Ok(()),
            };

            res?;
        }

        Ok(())
    }

    async fn collect_metrics(&self) {
        self.heartbeat().await;
        self.bandwidth().await;
        self.cpu_usage().await;
        self.loadavg().await;
        self.memory().await;
        self.disk_usage().await;
    }
}
