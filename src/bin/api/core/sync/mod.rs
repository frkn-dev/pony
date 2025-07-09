use std::sync::Arc;
use tokio::sync::RwLock;

use pony::Connection as Conn;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::MemoryCache;
use pony::NodeStorageOp;

use super::postgres::PgContext;

pub(crate) mod tasks;

#[derive(Clone)]
pub struct MemSync<N, C>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<MemoryCache<N, C>>>,
    pub db: PgContext,
}

impl<N, C> MemSync<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + From<Conn> + PartialEq,
{
    pub fn new(memory: Arc<RwLock<MemoryCache<N, C>>>, db: PgContext) -> Self {
        Self { memory, db }
    }
}
