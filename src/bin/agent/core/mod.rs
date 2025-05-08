use std::sync::Arc;
use tokio::sync::Mutex;

use pony::state::ConnBaseOp;
use pony::state::NodeStorage;
use pony::state::State;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;

mod http;
pub(crate) mod metrics;
pub mod service;
mod stats;
mod tasks;

pub struct Agent<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub state: Arc<Mutex<State<N, C>>>,
    pub subscriber: ZmqSubscriber,
    pub xray_stats_client: Arc<Mutex<StatsClient>>,
    pub xray_handler_client: Arc<Mutex<HandlerClient>>,
}

impl<N, C> Agent<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub fn new(
        state: Arc<Mutex<State<N, C>>>,
        subscriber: ZmqSubscriber,
        xray_stats_client: Arc<Mutex<StatsClient>>,
        xray_handler_client: Arc<Mutex<HandlerClient>>,
    ) -> Self {
        Self {
            state,
            subscriber,
            xray_stats_client,
            xray_handler_client,
        }
    }
}
