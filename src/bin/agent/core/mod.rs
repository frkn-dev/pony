use std::sync::Arc;
use tokio::sync::Mutex;

use pony::state::node::Node;
use pony::wireguard_op::WgApi;
use pony::xray_op::client::HandlerClient;
use pony::xray_op::client::StatsClient;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::Base as Connection;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::State;

mod http;
pub(crate) mod metrics;
pub mod service;
mod stats;
mod tasks;

pub type AgentState = State<Node, Connection>;

pub struct Agent<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    pub state: Arc<Mutex<State<N, C>>>,
    pub subscriber: ZmqSubscriber,
    pub xray_stats_client: Option<Arc<Mutex<StatsClient>>>,
    pub xray_handler_client: Option<Arc<Mutex<HandlerClient>>>,
    pub wg_client: Option<WgApi>,
}

impl<N, C> Agent<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    pub fn new(
        state: Arc<Mutex<State<N, C>>>,
        subscriber: ZmqSubscriber,
        xray_stats_client: Option<Arc<Mutex<StatsClient>>>,
        xray_handler_client: Option<Arc<Mutex<HandlerClient>>>,
        wg_client: Option<WgApi>,
    ) -> Self {
        Self {
            state,
            subscriber,
            xray_stats_client,
            xray_handler_client,
            wg_client,
        }
    }
}
