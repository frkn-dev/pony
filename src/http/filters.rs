use log::debug;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{Filter, Rejection};

use crate::http::handlers::AuthError;
use crate::DbContext;
use crate::NodeStorage;
use crate::State;
use crate::ZmqPublisher;

/// Provides authentication filter based on API token
pub fn auth(token: Arc<String>) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::<String>("authorization")
        .and_then(move |auth_header: String| {
            let token = token.clone();
            async move {
                debug!("{} - {}", auth_header, *token);
                if auth_header
                    .strip_prefix("Bearer ")
                    .map_or(false, |t| t == token.as_str())
                {
                    Ok(())
                } else {
                    Err(warp::reject::custom(AuthError("Unauthorized".to_string())))
                }
            }
        })
        .untuple_one()
}

/// Provides application state filter
pub fn with_state<T>(
    state: Arc<Mutex<State<T>>>,
) -> impl Filter<Extract = (Arc<Mutex<State<T>>>,), Error = std::convert::Infallible> + Clone
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    warp::any().map(move || state.clone())
}

/// Provides database context filter
pub fn db(
    db: DbContext,
) -> impl Filter<Extract = (DbContext,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

/// Provides zmq publisher filter
pub fn publisher(
    publisher: ZmqPublisher,
) -> impl Filter<Extract = (ZmqPublisher,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || publisher.clone())
}
