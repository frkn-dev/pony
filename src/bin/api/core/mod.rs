use std::collections::HashMap;

use pony::config::settings::ApiSettings;
use pony::state::node::Node;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Conn as Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::State;

use crate::core::clickhouse::ChContext;
use crate::core::sync::SyncState;

pub(crate) mod clickhouse;
pub(crate) mod http;
pub(crate) mod metrics;
pub(crate) mod postgres;
pub(crate) mod sync;
pub(crate) mod tasks;

pub type ApiState = State<HashMap<String, Vec<Node>>, Connection>;

pub struct Api<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
{
    pub state: SyncState<N, C>,
    pub ch: ChContext,
    pub publisher: ZmqPublisher,
    pub settings: ApiSettings,
}

impl<N, C> Api<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
{
    pub fn new(
        ch: ChContext,
        publisher: ZmqPublisher,
        state: SyncState<N, C>,
        settings: ApiSettings,
    ) -> Self {
        Self {
            ch,
            publisher,
            state,
            settings,
        }
    }
}
