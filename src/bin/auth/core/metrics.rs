use pony::memory::node::Node;
use pony::metrics::storage::HasMetrics;
use pony::metrics::storage::MetricBuffer;
use pony::ConnectionBaseOp;

use super::AuthService;

impl<C> HasMetrics for AuthService<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    fn metrics(&self) -> &MetricBuffer {
        &self.metrics
    }

    fn node_settings(&self) -> &Node {
        &self.node
    }
}
