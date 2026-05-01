use fcore::{ConnectionBaseOperations, HasMetrics, MetricBuffer, Node};

use super::service::Service;

impl<C> HasMetrics for Service<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static,
{
    fn metrics(&self) -> &MetricBuffer {
        &self.metrics
    }

    fn node_settings(&self) -> &Node {
        &self.node
    }
}
