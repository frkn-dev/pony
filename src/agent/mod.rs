pub mod service;
pub mod tasks;

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::HandlerClient;
use crate::NodeStorage;
use crate::State;
use crate::StatsClient;
use crate::ZmqSubscriber;

pub struct Agent<T>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    pub state: Arc<Mutex<State<T>>>,
    pub subscriber: ZmqSubscriber,
    pub xray_stats_client: Arc<Mutex<StatsClient>>,
    pub xray_handler_client: Arc<Mutex<HandlerClient>>,
}

impl<T> Agent<T>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    pub fn new(
        state: Arc<Mutex<State<T>>>,
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
