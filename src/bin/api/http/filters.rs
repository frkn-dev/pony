use std::sync::Arc;
use warp::Filter;

use pony::{
    Connection, ConnectionApiOperations, ConnectionBaseOperations, IpAddrMask, MetricStorage,
    NodeStorageOperations, SubscriptionOperations,
};

use super::super::sync::MemSync;

/// Provides application state filter
pub fn with_sync<T, C, S>(
    mem_sync: MemSync<T, C, S>,
) -> impl Filter<Extract = (MemSync<T, C, S>,), Error = std::convert::Infallible> + Clone
where
    T: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>,
    S: SubscriptionOperations + Send + Sync + Clone + 'static,
{
    warp::any().map(move || mem_sync.clone())
}

pub fn with_param_vec(
    param: Vec<u8>,
) -> impl Filter<Extract = (Vec<u8>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || param.clone())
}

pub fn with_param_ipaddrmask(
    param: IpAddrMask,
) -> impl Filter<Extract = (IpAddrMask,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || param.clone())
}

pub fn with_param_vec_string(
    param: Vec<String>,
) -> impl Filter<Extract = (Vec<String>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || param.clone())
}

pub fn with_metrics(
    metrics: Arc<MetricStorage>,
) -> impl Filter<Extract = (Arc<MetricStorage>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || metrics.clone())
}
