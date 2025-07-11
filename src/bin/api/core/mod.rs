use std::collections::HashMap;

use pony::config::settings::ApiSettings;
use pony::memory::node::Node;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::Publisher as ZmqPublisher;

use crate::core::clickhouse::ChContext;
use crate::core::sync::MemSync;

pub(crate) mod clickhouse;
pub(crate) mod http;
pub(crate) mod metrics;
pub(crate) mod postgres;
pub(crate) mod sync;
pub(crate) mod tasks;

pub type ApiState = MemoryCache<HashMap<String, Vec<Node>>, Connection>;

pub struct Api<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
{
    pub sync: MemSync<N, C>,
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
        sync: MemSync<N, C>,
        settings: ApiSettings,
    ) -> Self {
        Self {
            ch,
            publisher,
            sync,
            settings,
        }
    }
}
