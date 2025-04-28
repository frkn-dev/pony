use async_trait::async_trait;
use log::debug;
use log::error;
use log::warn;
use std::error::Error;
use std::sync::Arc;

use super::Agent;
use crate::metrics::bandwidth::bandwidth_metrics;
use crate::metrics::cpuusage::cpu_metrics;
use crate::metrics::heartbeat::heartbeat_metrics;
use crate::metrics::loadavg::loadavg_metrics;
use crate::metrics::memory::mem_metrics;
use crate::metrics::xray::xray_stat_metrics;
use crate::metrics::xray::xray_user_metrics;
use crate::xray_op::client::HandlerActions;
use crate::xray_op::stats;
use crate::xray_op::stats::Prefix;
use crate::Action;
use crate::Message;
use crate::MetricType;
use crate::NodeStorage;
use crate::Topic;
use crate::User;
use crate::UserStorage;

#[async_trait]
pub trait Tasks {
    async fn send_metrics(
        &self,
        carbon_address: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn collect_stats(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
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
        let users = state.users.clone();

        let node = state.nodes.get_node();

        if let Some(node) = node {
            let bandwidth: Vec<MetricType> =
                bandwidth_metrics(&node.env, &node.hostname, &node.iface);
            let cpuusage: Vec<MetricType> = cpu_metrics(&node.env, &node.hostname);
            let loadavg: Vec<MetricType> = loadavg_metrics(&node.env, &node.hostname);
            let memory: Vec<MetricType> = mem_metrics(&node.env, &node.hostname);
            let heartbeat: Vec<MetricType> = heartbeat_metrics(&node.env, node.uuid);
            let xray_stat: Vec<MetricType> = xray_stat_metrics(node.clone());
            let users_stat: Vec<MetricType> = xray_user_metrics(users, &node.env, &node.hostname);

            metrics.extend(bandwidth);
            metrics.extend(cpuusage);
            metrics.extend(loadavg);
            metrics.extend(memory);
            metrics.extend(xray_stat);
            metrics.extend(users_stat);
            metrics.extend(heartbeat);
        }

        debug!("Total metrics collected: {}", metrics.len());

        metrics
    }

    async fn collect_stats(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tasks = vec![];
        debug!("Running xray stat job");

        let user_ids = {
            let state = self.state.lock().await;
            state.users.keys().cloned().collect::<Vec<_>>()
        };

        for user_id in user_ids {
            let stats_client = Arc::clone(&self.xray_stats_client);
            let state = self.state.clone();

            tasks.push(tokio::spawn(async move {
                if let Ok(user_stat) =
                    stats::get_user_stats(stats_client, Prefix::UserPrefix(user_id)).await
                {
                    let mut state = state.lock().await;
                    if let Err(e) = state.update_user_downlink(user_id, user_stat.downlink) {
                        error!("Failed to update user downlink: {}", e);
                    }
                    if let Err(e) = state.update_user_uplink(user_id, user_stat.uplink) {
                        error!("Failed to update user uplink: {}", e);
                    }
                    if let Err(e) = state.update_user_online(user_id, user_stat.online) {
                        error!("Failed to update user online: {}", e);
                    }
                } else {
                    error!("Failed to fetch user stats for {}", user_id);
                }
            }));
        }

        if let Some(node) = {
            let state = self.state.lock().await;
            state.nodes.get_node()
        } {
            let node_tags = node.inbounds.keys().cloned().collect::<Vec<_>>();

            for tag in node_tags {
                let stats_client = self.xray_stats_client.clone();
                let handler_client = self.xray_handler_client.clone();
                let state = self.state.clone();
                let env = node.env.clone();

                tasks.push(tokio::spawn(async move {
                    if let Ok(inbound_stat) = stats::get_inbound_stats(
                        stats_client,
                        handler_client,
                        Prefix::InboundPrefix(tag.clone()),
                    )
                    .await
                    {
                        let mut state = state.lock().await;
                        if let Err(e) = state.nodes.update_node_downlink(
                            tag.clone(),
                            inbound_stat.downlink,
                            env.clone(),
                            node.uuid,
                        ) {
                            error!("Failed to update inbound downlink: {}", e);
                        }
                        if let Err(e) = state.nodes.update_node_uplink(
                            tag.clone(),
                            inbound_stat.uplink,
                            env.clone(),
                            node.uuid,
                        ) {
                            error!("Failed to update inbound uplink: {}", e);
                        }
                        if let Err(e) = state.nodes.update_node_user_count(
                            tag.clone(),
                            inbound_stat.user_count,
                            env.clone(),
                            node.uuid,
                        ) {
                            error!("Failed to update user_count: {}", e);
                        }
                    }
                }));
            }
        }

        let results = futures::future::join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                error!("Task panicked: {:?}", e);
            }
        }

        Ok(())
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
                let user_id = msg.user_id;

                let user = User::new(msg.trial, msg.limit, msg.env.clone(), msg.password.clone());

                debug!("USER {:?}", user);

                match self
                    .xray_handler_client
                    .create_all(msg.user_id, user.password.clone())
                    .await
                {
                    Ok(_) => {
                        let mut state = self.state.lock().await;

                        state
                            .add_or_update_user(user_id.clone(), user)
                            .map_err(|err| {
                                error!("Failed to add user {}: {:?}", msg.user_id, err);
                                format!("Failed to add user {}", msg.user_id).into()
                            })
                    }
                    Err(err) => {
                        error!("Failed to create user {}: {:?}", msg.user_id, err);
                        Err(format!("Failed to create user {}", msg.user_id).into())
                    }
                }
            }
            Action::Delete => {
                if let Err(e) = self.xray_handler_client.remove_all(msg.user_id).await {
                    return Err(format!("Couldn't remove users from Xray: {}", e).into());
                } else {
                    let mut state = self.state.lock().await;
                    let _ = state.remove_user(msg.user_id);
                }

                Ok(())
            }
        }
    }
}
