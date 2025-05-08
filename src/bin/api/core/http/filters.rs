use warp::Filter;

use pony::state::Conn;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::NodeStorage;
use pony::state::SyncState;
use pony::zmq::publisher::Publisher as ZmqPublisher;

/// Provides application state filter
pub fn with_state<T, C>(
    sync_state: SyncState<T, C>,
) -> impl Filter<Extract = (SyncState<T, C>,), Error = std::convert::Infallible> + Clone
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    warp::any().map(move || sync_state.clone())
}

/// Provides zmq publisher filter
pub fn publisher(
    publisher: ZmqPublisher,
) -> impl Filter<Extract = (ZmqPublisher,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || publisher.clone())
}
