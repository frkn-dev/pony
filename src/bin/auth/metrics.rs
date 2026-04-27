use pony::{ConnectionBaseOperations, HasMetrics, MetricBuffer, Node};

use super::auth::AuthService;

impl<C> HasMetrics for AuthService<C>
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
