use pony::metrics::cpuusage::cpu_metrics;
use pony::metrics::heartbeat::heartbeat_metrics;
use pony::metrics::loadavg::loadavg_metrics;
use pony::metrics::memory::mem_metrics;
use pony::metrics::metrics::MetricType;
use pony::metrics::Metrics;
use pony::state::Conn;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::NodeStorage;

use crate::Api;

#[async_trait::async_trait]
impl<T, C> Metrics<T> for Api<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static + From<Conn>,
{
    async fn collect_metrics<M>(&self) -> Vec<MetricType>
    where
        T: NodeStorage + Sync + Send + Clone + 'static,
    {
        let mut metrics = Vec::new();

        let env = &self.settings.node.env;
        let uuid = self.settings.node.uuid;

        if let Some(hostname) = &self.settings.node.hostname {
            metrics.extend(cpu_metrics(env, hostname));
            metrics.extend(loadavg_metrics(env, hostname));
            metrics.extend(mem_metrics(env, hostname));
            metrics.extend(heartbeat_metrics(env, &uuid, hostname));
        } else {
            log::warn!("Hostname is not set, skipping metrics collection");
        }

        log::debug!("Total metrics collected: {}", metrics.len());
        metrics
    }
}
