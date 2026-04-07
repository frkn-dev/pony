use pony::Publisher;
use std::sync::Arc;
use tokio::sync::RwLock;

use pony::Connection as Conn;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

use super::postgres::PgContext;
use super::Cache;

pub(crate) mod tasks;

#[derive(Clone)]
pub struct MemSync<N, C, S>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<Cache<N, C, S>>>,
    pub db: PgContext,
    pub publisher: Publisher,
}

impl<N, C, S> MemSync<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + From<Conn> + PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub fn new(memory: Arc<RwLock<Cache<N, C, S>>>, db: PgContext, publisher: Publisher) -> Self {
        Self {
            memory,
            db,
            publisher,
        }
    }
}
