use async_trait::async_trait;
use futures::future::join_all;
use std::sync::Arc;
use tokio::task::JoinHandle;

use pony::state::ConnBase;
use pony::state::ConnBaseOp;
use pony::state::ConnStorageBase;
use pony::state::NodeStorage;
use pony::state::Tag;
use pony::xray_op::client::HandlerActions;
use pony::xray_op::stats::Prefix;
use pony::xray_op::stats::StatOp;
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::zmq::Topic;
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
    T: NodeStorage + Send + Sync + Clone,
    C: ConnBaseOp + Send + Sync + Clone + 'static + From<ConnBase>,
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
                        log::warn!("ZMQ: Skipping init for another node: {}", uuid);
                        continue;
                    }
                    Topic::Updates(env) if env != topic1 => {
                        log::warn!("ZMQ: Skipping update for another env: {}", env);
                        continue;
                    }
                    Topic::Unknown(raw) => {
                        log::warn!("ZMQ: Unknown topic: {}", raw);
                        continue;
                    }
                    Topic::All => {
                        log::debug!("ZMQ: message for All topic recieved");
                    }
                    _ => {}
                }

                if let Ok(message) = serde_json::from_str::<Message>(payload) {
                    if let Err(err) = self.handle_message(message).await {
                        log::error!("ZMQ SUB: Failed to handle message: {}", err);
                    }
                } else {
                    log::error!("ZMQ SUB: Failed to parse payload: {}", payload);
                }
            }
        }
    }

    async fn handle_message(&self, msg: Message) -> Result<()> {
        match msg.action {
            Action::Create => {
                let conn_id = msg.conn_id;
                let conn = ConnBase::new(msg.tag, msg.password.clone());

                match self
                    .xray_handler_client
                    .create(&msg.conn_id, msg.tag, conn.password.clone())
                    .await
                {
                    Ok(_) => {
                        let mut mem = self.state.lock().await;

                        mem.connections
                            .add(&conn_id, conn.into())
                            .map(|_| ())
                            .map_err(|err| {
                                log::error!("Failed to add conn {}: {:?}", msg.conn_id, err);
                                format!("Failed to add conn {}", msg.conn_id).into()
                            })
                    }
                    Err(err) => {
                        log::error!("Failed to create conn {}: {:?}", msg.conn_id, err);
                        Err(
                            PonyError::Custom(format!("Failed to create conn {}", msg.conn_id))
                                .into(),
                        )
                    }
                }
            }

            Action::Delete => {
                if let Err(e) = self
                    .xray_handler_client
                    .remove(&msg.conn_id, msg.tag, msg.password)
                    .await
                {
                    return Err(PonyError::Custom(format!(
                        "Couldn't remove connections from Xray: {}",
                        e
                    ))
                    .into());
                } else {
                    let mut state = self.state.lock().await;

                    let _ = state.connections.remove(&msg.conn_id);
                }

                Ok(())
            }
            Action::ResetStat => {
                if let Err(e) = self.reset_stat(&msg.conn_id).await {
                    return Err(PonyError::Custom(format!(
                        "Couldn't reset stat for connection: {}",
                        e
                    ))
                    .into());
                } else {
                    log::debug!("Reset stat for {}", &msg.conn_id);
                    Ok(())
                }
            }
        }
    }
}

impl<T, C> Agent<T, C>
where
    T: NodeStorage + Send + Sync + Clone,
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    async fn collect_conn_stats(self: Arc<Self>, conn_id: uuid::Uuid) -> Result<()> {
        let conn_stat = self.conn_stats(Prefix::ConnPrefix(conn_id)).await?;
        let mut state = self.state.lock().await;
        let _ = state
            .connections
            .update_downlink(&conn_id, conn_stat.downlink);
        let _ = state.connections.update_uplink(&conn_id, conn_stat.uplink);
        let _ = state.connections.update_online(&conn_id, conn_stat.online);
        Ok(())
    }

    async fn collect_inbound_stats(
        self: Arc<Self>,
        tag: Tag,
        env: String,
        node_uuid: uuid::Uuid,
    ) -> Result<()> {
        let inbound_stat = self
            .inbound_stats(Prefix::InboundPrefix(tag.clone()))
            .await?;
        let mut state = self.state.lock().await;
        let _ = state
            .nodes
            .update_node_downlink(&tag, inbound_stat.downlink, &env, &node_uuid);
        let _ = state
            .nodes
            .update_node_uplink(&tag, inbound_stat.uplink, &env, &node_uuid);
        let _ = state
            .nodes
            .update_node_conn_count(&tag, inbound_stat.conn_count, &env, &node_uuid);
        Ok(())
    }

    pub async fn collect_stats(self: Arc<Self>) -> Result<()> {
        log::debug!("Running xray stat job");
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();

        let conn_ids = {
            let state = self.state.lock().await;
            state.connections.keys().cloned().collect::<Vec<_>>()
        };

        for conn_id in conn_ids {
            let agent = self.clone();
            tasks.push(tokio::spawn(async move {
                if let Err(e) = agent.collect_conn_stats(conn_id).await {
                    log::error!("Failed to collect stats for connection {}: {}", conn_id, e);
                }
            }));
        }

        if let Some(node) = {
            let state = self.state.lock().await;
            state.nodes.get_self()
        } {
            let node_tags = node.inbounds.keys().cloned().collect::<Vec<_>>();
            for tag in node_tags {
                let agent = self.clone();
                let env = node.env.clone();
                let node_uuid = node.uuid;
                tasks.push(tokio::spawn(async move {
                    if let Err(e) = agent
                        .collect_inbound_stats(tag.clone(), env, node_uuid)
                        .await
                    {
                        log::error!("Failed to collect stats for inbound {}: {}", tag.clone(), e);
                    }
                }));
            }
        }

        let results = join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Task panicked: {:?}", e);
            }
        }

        Ok(())
    }
}
