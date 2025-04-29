use std::sync::Arc;
use tokio::sync::Mutex;

use pony::state::connection::ConnBaseOp;
use pony::state::state::NodeStorage;
use pony::state::state::State;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;

mod http;
pub mod service;
mod stats;
mod tasks;

pub struct Agent<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub state: Arc<Mutex<State<T, C>>>,
    pub subscriber: ZmqSubscriber,
    pub xray_stats_client: Arc<Mutex<StatsClient>>,
    pub xray_handler_client: Arc<Mutex<HandlerClient>>,
}

impl<T, C> Agent<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub fn new(
        state: Arc<Mutex<State<T, C>>>,
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
