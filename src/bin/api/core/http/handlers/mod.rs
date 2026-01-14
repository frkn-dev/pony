pub mod connection;
pub mod node;
pub mod sub;

use warp::http::StatusCode;

use pony::http::ResponseMessage;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;

use crate::core::sync::MemSync;

// GET /healthcheck
pub async fn healthcheck_handler<N, C>(
    _state: MemSync<N, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
{
    let response = ResponseMessage::<Option<uuid::Uuid>> {
        status: 200,
        message: "Ok".to_string(),
        response: None,
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}
