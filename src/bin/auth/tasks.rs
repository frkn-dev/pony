use async_trait::async_trait;
use rkyv::{AlignedVec, Deserialize, Infallible};
use tokio::time::Duration;

use fcore::{
    Action, BaseConnection as Connection, ConnectionBaseOperations,
    ConnectionStorageBaseOperations, Error, Message, Metrics, Proto, Result, Topic,
};

use super::service::Service;

#[async_trait]
pub trait Tasks {
    async fn run_subscriber(&self) -> Result<()>;
    async fn handle_messages_batch(&self, msg: Vec<Message>) -> Result<()>;
    async fn collect_metrics(&self);
}

#[async_trait]
impl<C> Tasks for Service<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static + From<Connection>,
{
    async fn run_subscriber(&self) -> Result<()> {
        let sub = self.subscriber.clone();
        let node_uuid = self.node.uuid;

        loop {
            let Some((topic_bytes, payload_bytes)) = sub.recv().await else {
                tracing::warn!("SUB: No multipart message received");
                continue;
            };

            let topic_str = std::str::from_utf8(&topic_bytes)
                .map_err(|_| Error::Custom("Invalid UTF8 topic".into()))?;

            let topic = match topic_str.parse::<Topic>() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("SUB: Failed to parse topic '{}': {}", topic_str, e);
                    continue;
                }
            };

            tracing::debug!("SUB: Received topic: {:?}", topic);

            match &topic {
                Topic::Auth => {
                    tracing::debug!("SUB: Processing Auth message");
                }

                Topic::Metrics => {
                    tracing::trace!("SUB: Ignoring Metrics topic");
                    continue;
                }

                Topic::Updates(env) => {
                    tracing::trace!("SUB: Ignoring update for env: {}", env);
                    continue;
                }

                Topic::Init(uuid) if uuid != &node_uuid => {
                    tracing::trace!("SUB: Skipping init for another node: {}", uuid);
                    continue;
                }
                _ => {
                    tracing::debug!("SUB: Accepted for processing: {:?}", topic);
                }
            }

            if payload_bytes.is_empty() {
                continue;
            }

            let messages: Option<Vec<Message>> = {
                let mut aligned = AlignedVec::new();
                aligned.extend_from_slice(&payload_bytes);

                match rkyv::check_archived_root::<Vec<Message>>(&aligned) {
                    Ok(archived) => archived.deserialize(&mut Infallible).ok(),
                    Err(e) => {
                        tracing::error!("SUB: Invalid rkyv root: {:?}", e);
                        None
                    }
                }
            };

            if let Some(msgs) = messages {
                if let Err(err) = self.handle_messages_batch(msgs).await {
                    tracing::error!("SUB: Failed to handle messages: {}", err);
                }
            }

            tokio::time::sleep(Duration::from_millis(1)).await;
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
