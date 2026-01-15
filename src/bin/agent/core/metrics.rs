use chrono::Utc;

use pony::metrics::bandwidth::bandwidth_metrics;
use pony::metrics::cpuusage::cpu_metrics;
use pony::metrics::heartbeat::heartbeat_metrics;
use pony::metrics::loadavg::loadavg_metrics;
use pony::metrics::memory::mem_metrics;
use pony::metrics::metrics::Metric;
use pony::metrics::metrics::MetricType;
use pony::metrics::xray::xray_conn_metrics;
use pony::metrics::xray::xray_stat_metrics;
use pony::metrics::Metrics;
use pony::Connection;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

use crate::core::Agent;

#[async_trait::async_trait]
impl<T, C, S> Metrics<T> for Agent<T, C, S>
where
    T: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static + From<Connection>,
    S: Send + Sync + Clone + 'static + std::cmp::PartialEq + SubscriptionOp,
{
    async fn collect_metrics<M>(&self) -> Vec<MetricType>
    where
        T: NodeStorageOp + Sync + Send + Clone + 'static,
    {
        let mut metrics: Vec<MetricType> = Vec::new();
        let mem = self.memory.read().await;
        let connections = mem.connections.clone();

        let node = mem.nodes.get_self();

        if let Some(node) = node {
            let bandwidth: Vec<MetricType> =
                bandwidth_metrics(&node.env, &node.hostname, &node.interface);
            let cpuusage: Vec<MetricType> = cpu_metrics(&node.env, &node.hostname);
            let loadavg: Vec<MetricType> = loadavg_metrics(&node.env, &node.hostname);
            let memory: Vec<MetricType> = mem_metrics(&node.env, &node.hostname);

            let xray_stat: Vec<MetricType> = xray_stat_metrics(node.clone());
            let connections_stat: Vec<MetricType> =
                xray_conn_metrics(connections, &node.env, &node.hostname);

            metrics.extend(bandwidth);
            metrics.extend(cpuusage);
            metrics.extend(loadavg);
            metrics.extend(memory);
            metrics.extend(xray_stat);
            metrics.extend(connections_stat);
        }

        log::debug!("Total metrics collected: {}", metrics.len());

        metrics
    }

    async fn collect_hb_metrics<M>(&self) -> MetricType {
        let mem = self.memory.read().await;
        let node = mem.nodes.get_self();
        if let Some(node) = node {
            heartbeat_metrics(&node.env, &node.uuid, &node.hostname)
        } else {
            MetricType::F64(Metric::new(
                "hb.unknown".into(),
                0.0,
                Utc::now().timestamp(),
            ))
        }
    }
}
