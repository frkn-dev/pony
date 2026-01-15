use chrono::Utc;

use pony::metrics::cpuusage::cpu_metrics;
use pony::metrics::heartbeat::heartbeat_metrics;
use pony::metrics::loadavg::loadavg_metrics;
use pony::metrics::memory::mem_metrics;
use pony::metrics::metrics::Metric;
use pony::metrics::metrics::MetricType;
use pony::metrics::Metrics;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

use crate::Api;

#[async_trait::async_trait]
impl<N, C, S> Metrics<N> for Api<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Send
        + Sync
        + Clone
        + 'static
        + From<Connection>
        + std::cmp::PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    async fn collect_metrics<M>(&self) -> Vec<MetricType>
    where
        N: NodeStorageOp + Sync + Send + Clone + 'static,
    {
        let mut metrics = Vec::new();

        let env = &self.settings.node.env;
        let _uuid = self.settings.node.uuid;

        if let Some(hostname) = &self.settings.node.hostname {
            metrics.extend(cpu_metrics(env, hostname));
            metrics.extend(loadavg_metrics(env, hostname));
            metrics.extend(mem_metrics(env, hostname));
        } else {
            log::warn!("Hostname is not set, skipping metrics collection");
        }

        log::debug!("Total metrics collected: {}", metrics.len());
        metrics
    }

    async fn collect_hb_metrics<M>(&self) -> MetricType {
        if let Some(hostname) = &self.settings.node.hostname {
            heartbeat_metrics(&self.settings.node.env, &self.settings.node.uuid, &hostname)
        } else {
            MetricType::F64(Metric::new(
                "hb.unknown".into(),
                0.0,
                Utc::now().timestamp(),
            ))
        }
    }
}
