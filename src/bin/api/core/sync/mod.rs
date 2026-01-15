use std::sync::Arc;
use tokio::sync::RwLock;

use pony::Connection as Conn;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

use super::postgres::PgContext;

pub(crate) mod tasks;

#[derive(Clone)]
pub struct MemSync<N, C, S>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<MemoryCache<N, C, S>>>,
    pub db: PgContext,
}

impl<N, C, S> MemSync<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + From<Conn> + PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub fn new(memory: Arc<RwLock<MemoryCache<N, C, S>>>, db: PgContext) -> Self {
        Self { memory, db }
    }
}
