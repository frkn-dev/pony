use async_trait::async_trait;
use futures::future::join_all;
use log::debug;
use log::error;
use log::warn;
use std::error::Error;
use std::sync::Arc;
use tokio::task::JoinHandle;

use super::Agent;
use crate::metrics::bandwidth::bandwidth_metrics;
use crate::metrics::cpuusage::cpu_metrics;
use crate::metrics::heartbeat::heartbeat_metrics;
use crate::metrics::loadavg::loadavg_metrics;
use crate::metrics::memory::mem_metrics;
use crate::metrics::metrics::MetricType;
use crate::metrics::xray::*;
use crate::state::connection::Conn;
use crate::state::state::ConnStorage;
use crate::state::state::NodeStorage;
use crate::state::tag::Tag;
use crate::xray_op::client::HandlerActions;
use crate::xray_op::stats::Prefix;
use crate::xray_op::stats::StatOp;
use crate::zmq::message::Action;
use crate::zmq::message::Message;
use crate::zmq::Topic;

#[async_trait]
pub trait Tasks {
    async fn send_metrics(
        &self,
        carbon_address: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn collect_metrics<M>(&self) -> Vec<MetricType>;
    async fn run_subscriber(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn handle_message(&self, msg: Message) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
impl<T: NodeStorage + Send + Sync + Clone> Tasks for Agent<T> {
    async fn send_metrics(
        &self,
        carbon_address: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let metrics = self.collect_metrics::<T>().await;

        for metric in metrics {
            match metric {
                MetricType::F32(m) => m.send(&carbon_address).await?,
                MetricType::F64(m) => m.send(&carbon_address).await?,
                MetricType::I64(m) => m.send(&carbon_address).await?,
                MetricType::U64(m) => m.send(&carbon_address).await?,
                MetricType::U8(m) => m.send(&carbon_address).await?,
            }
        }
        Ok(())
    }

    async fn collect_metrics<M>(&self) -> Vec<MetricType>
    where
        T: NodeStorage + Sync + Send + Clone + 'static,
    {
        let mut metrics: Vec<MetricType> = Vec::new();
        let state = self.state.lock().await;
        let connections = state.connections.clone();

        let node = state.nodes.get();

        if let Some(node) = node {
            let bandwidth: Vec<MetricType> =
                bandwidth_metrics(&node.env, &node.hostname, &node.interface);
            let cpuusage: Vec<MetricType> = cpu_metrics(&node.env, &node.hostname);
            let loadavg: Vec<MetricType> = loadavg_metrics(&node.env, &node.hostname);
            let memory: Vec<MetricType> = mem_metrics(&node.env, &node.hostname);
            let heartbeat: Vec<MetricType> = heartbeat_metrics(&node.env, node.uuid);
            let xray_stat: Vec<MetricType> = xray_stat_metrics(node.clone());
            let connections_stat: Vec<MetricType> =
                xray_conn_metrics(connections, &node.env, &node.hostname);

            metrics.extend(bandwidth);
            metrics.extend(cpuusage);
            metrics.extend(loadavg);
            metrics.extend(memory);
            metrics.extend(xray_stat);
            metrics.extend(connections_stat);
            metrics.extend(heartbeat);
        }

        debug!("Total metrics collected: {}", metrics.len());

        metrics
    }

    async fn run_subscriber(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sub = self.subscriber.clone();

        let topic0 = self.subscriber.topics[0].clone();
        let topic1 = self.subscriber.topics[1].clone();

        loop {
            if let Some(data) = sub.recv().await {
                let mut parts = data.splitn(2, ' ');
                let topic_str = parts.next().unwrap_or("");
                let payload = parts.next().unwrap_or("");

                match Topic::from_raw(topic_str) {
                    Topic::Init(uuid) if uuid != topic0 => {
                        warn!("ZMQ: Skipping init for another node: {}", uuid);
                        continue;
                    }
                    Topic::Updates(env) if env != topic1 => {
                        warn!("ZMQ: Skipping update for another env: {}", env);
                        continue;
                    }
                    Topic::Unknown(raw) => {
                        warn!("ZMQ: Unknown topic: {}", raw);
                        continue;
                    }
                    _ => {}
                }

                if let Ok(message) = serde_json::from_str::<Message>(payload) {
                    if let Err(err) = self.handle_message(message).await {
                        error!("ZMQ SUB: Failed to handle message: {}", err);
                    }
                } else {
                    error!("ZMQ SUB: Failed to parse payload: {}", payload);
                }
            }
        }
    }

    async fn handle_message(&self, msg: Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        match msg.action {
            Action::Create | Action::Update => {
                let conn_id = msg.conn_id;

                let conn = Conn::new(msg.trial, msg.limit, msg.env.clone(), msg.password.clone());

                match self
                    .xray_handler_client
                    .create_all(&msg.conn_id, conn.password.clone())
                    .await
                {
                    Ok(_) => {
                        let mut state = self.state.lock().await;

                        state
                            .connections
                            .add_or_update(&conn_id.clone(), conn)
                            .map_err(|err| {
                                error!("Failed to add conn {}: {:?}", msg.conn_id, err);
                                format!("Failed to add conn {}", msg.conn_id).into()
                            })
                    }
                    Err(err) => {
                        error!("Failed to create conn {}: {:?}", msg.conn_id, err);
                        Err(format!("Failed to create conn {}", msg.conn_id).into())
                    }
                }
            }
            Action::Delete => {
                if let Err(e) = self.xray_handler_client.remove_all(&msg.conn_id).await {
                    return Err(format!("Couldn't remove connections from Xray: {}", e).into());
                } else {
                    let mut state = self.state.lock().await;

                    let _ = state.connections.remove(&msg.conn_id);
                }

                Ok(())
            }
        }
    }
}

impl<T: NodeStorage + Send + Sync + Clone> Agent<T> {
    async fn collect_conn_stats(
        self: Arc<Self>,
        conn_id: uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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

    pub async fn collect_stats(self: Arc<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
        debug!("Running xray stat job");
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();

        let conn_ids = {
            let state = self.state.lock().await;
            state.connections.keys().cloned().collect::<Vec<_>>()
        };

        for conn_id in conn_ids {
            let agent = self.clone();
            tasks.push(tokio::spawn(async move {
                if let Err(e) = agent.collect_conn_stats(conn_id).await {
                    error!("Failed to collect stats for connection {}: {}", conn_id, e);
                }
            }));
        }

        if let Some(node) = {
            let state = self.state.lock().await;
            state.nodes.get()
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
                        error!("Failed to collect stats for inbound {}: {}", tag.clone(), e);
                    }
                }));
            }
        }

        let results = join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                error!("Task panicked: {:?}", e);
            }
        }

        Ok(())
    }
}
