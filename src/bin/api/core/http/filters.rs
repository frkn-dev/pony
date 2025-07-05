use warp::Filter;

use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Conn as Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;

use crate::SyncState;

/// Provides application state filter
pub fn with_state<T, C>(
    sync_state: SyncState<T, C>,
) -> impl Filter<Extract = (SyncState<T, C>,), Error = std::convert::Infallible> + Clone
where
    T: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp + ConnectionBaseOp + Sync + Send + Clone + 'static + From<Connection>,
{
    warp::any().map(move || sync_state.clone())
}

/// Provides zmq publisher filter
pub fn publisher(
    publisher: ZmqPublisher,
) -> impl Filter<Extract = (ZmqPublisher,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || publisher.clone())
}
