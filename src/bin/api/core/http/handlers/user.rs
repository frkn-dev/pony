use warp::http::StatusCode;

use pony::http::requests::UserIdQueryParam;
use pony::http::ResponseMessage;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::ConnectionStorageApiOp;
use pony::NodeStorageOp;
use pony::Tag;

use crate::core::sync::MemSync;

// GET /user/stat?user_id=<>
pub async fn user_conn_stat_handler<N, C>(
    user_req: UserIdQueryParam,
    memory: MemSync<N, C>,
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
    log::debug!("Received: {:?}", user_req);

    let mem = memory.memory.read().await;
    let mut result: Vec<(uuid::Uuid, ConnectionStat, Tag, ConnectionStatus, i32, bool)> =
        Vec::new();

    if let Some(connections) = mem.connections.get_by_user_id(&user_req.user_id) {
        for (conn_id, conn) in connections {
            let tag = conn.get_proto().proto();

            let stat = ConnectionStat {
                online: conn.get_online(),
                downlink: conn.get_downlink(),
                uplink: conn.get_uplink(),
            };

            result.push((
                conn_id,
                stat,
                tag,
                conn.get_status(),
                conn.get_limit(),
                conn.get_trial(),
            ));
        }

        let response = ResponseMessage {
            status: StatusCode::OK.as_u16(),
            message: "List of user connection statistics".to_string(),
            response: result,
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<
            Option<Vec<(uuid::Uuid, ConnectionStat, Tag, ConnectionStatus, i32, bool)>>,
        > {
            status: StatusCode::NOT_FOUND.as_u16(),
            message: "Connections not found".to_string(),
            response: Some(vec![]),
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}
