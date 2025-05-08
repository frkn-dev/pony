use pony::metrics::bandwidth::bandwidth_metrics;
use pony::metrics::cpuusage::cpu_metrics;
use pony::metrics::heartbeat::heartbeat_metrics;
use pony::metrics::loadavg::loadavg_metrics;
use pony::metrics::memory::mem_metrics;
use pony::metrics::metrics::MetricType;
use pony::metrics::xray::xray_conn_metrics;
use pony::metrics::xray::xray_stat_metrics;
use pony::metrics::Metrics;
use pony::state::Conn;
use pony::state::ConnBaseOp;
use pony::state::NodeStorage;

use crate::core::Agent;

#[async_trait::async_trait]
impl<T, C> Metrics<T> for Agent<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + Send + Sync + Clone + 'static + From<Conn>,
{
    async fn collect_metrics<M>(&self) -> Vec<MetricType>
    where
        T: NodeStorage + Sync + Send + Clone + 'static,
    {
        let mut metrics: Vec<MetricType> = Vec::new();
        let state = self.state.lock().await;
        let connections = state.connections.clone();

        let node = state.nodes.get_self();

        if let Some(node) = node {
            let bandwidth: Vec<MetricType> =
                bandwidth_metrics(&node.env, &node.hostname, &node.interface);
            let cpuusage: Vec<MetricType> = cpu_metrics(&node.env, &node.hostname);
            let loadavg: Vec<MetricType> = loadavg_metrics(&node.env, &node.hostname);
            let memory: Vec<MetricType> = mem_metrics(&node.env, &node.hostname);
            let heartbeat: Vec<MetricType> =
                heartbeat_metrics(&node.env, &node.uuid, &node.hostname);
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

        log::debug!("Total metrics collected: {}", metrics.len());

        metrics
    }
}
