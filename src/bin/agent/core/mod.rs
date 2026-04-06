use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use pony::memory::connection::Connections;
use pony::memory::node::Node;
use pony::metrics::storage::MetricStorage;
use pony::wireguard_op::WgApi;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::ConnectionBaseOp;

mod http;
pub(crate) mod metrics;
pub mod service;
mod snapshot;
mod stats;
mod tasks;

pub struct Agent<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<Connections<C>>>,
    pub node: Node,
    pub metrics: Arc<MetricStorage>,
    pub subscriber: ZmqSubscriber,
    pub xray_stats_client: Option<Arc<Mutex<StatsClient>>>,
    pub xray_handler_client: Option<Arc<Mutex<HandlerClient>>>,
    pub wg_client: Option<WgApi>,
}

impl<C> Agent<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    pub fn new(
        node: Node,
        metrics: Arc<MetricStorage>,
        subscriber: ZmqSubscriber,
        xray_stats_client: Option<Arc<Mutex<StatsClient>>>,
        xray_handler_client: Option<Arc<Mutex<HandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Self {
        let memory = Arc::new(RwLock::new(Connections::default()));
        Self {
            memory,
            node,
            metrics,
            subscriber,
            xray_stats_client,
            xray_handler_client,
            wg_client,
        }
    }
}
