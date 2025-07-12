use warp::Filter;

use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;

use crate::core::clickhouse::ChContext;
use crate::MemSync;

/// Provides application state filter
pub fn with_state<T, C>(
    mem_sync: MemSync<T, C>,
) -> impl Filter<Extract = (MemSync<T, C>,), Error = std::convert::Infallible> + Clone
where
    T: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp + ConnectionBaseOp + Sync + Send + Clone + 'static + From<Connection>,
{
    warp::any().map(move || mem_sync.clone())
}

/// Provides ckickhouse filter
pub fn with_ch(
    ch: ChContext,
) -> impl Filter<Extract = (ChContext,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || ch.clone())
}

/// Provides zmq publisher filter
pub fn publisher(
    publisher: ZmqPublisher,
) -> impl Filter<Extract = (ZmqPublisher,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || publisher.clone())
}
