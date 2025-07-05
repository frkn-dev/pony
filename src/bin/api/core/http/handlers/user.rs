use pony::http::requests::UserUpdateReq;
use pony::http::IdResponse;
use warp::http::StatusCode;

use pony::http::requests::UserReq;

use pony::http::requests::UserIdQueryParam;
use pony::http::ResponseMessage;
use pony::state::storage::connection::ApiOp;
use pony::state::user::User;
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Conn as Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::Tag;

use crate::core::sync::tasks::SyncOp;
use crate::core::sync::SyncState;

// POST /user
pub async fn post_user_register_handler<N, C>(
    user_req: UserReq,
    state: SyncState<N, C>,
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
    Connection: From<C>,
{
    log::debug!("Received user registration request: {:?}", user_req);

    let user_id = uuid::Uuid::new_v4();
    let user: User = user_req.into();

    match SyncOp::add_user(&state, &user_id, user.clone()).await {
        Ok(StorageOperationStatus::NotModified(id)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage {
                status: StatusCode::NOT_MODIFIED.as_u16(),
                message: format!("User not modified  with id {}", id),
                response: Some(IdResponse { id: id }),
            }),
            StatusCode::NOT_MODIFIED,
        )),
        Ok(StorageOperationStatus::Ok(_)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage {
                status: StatusCode::OK.as_u16(),
                message: format!("User registered with id {}", user_id),
                response: Some(IdResponse { id: user_id }),
            }),
            StatusCode::OK,
        )),
        Ok(StorageOperationStatus::AlreadyExist(id)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::CONFLICT.as_u16(),
                message: format!("User already exists, {0}", user.username),
                response: Some(IdResponse { id: id }),
            }),
            StatusCode::CONFLICT,
        )),
        Ok(StorageOperationStatus::DeletedPreviously(id)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::CONFLICT.as_u16(),
                message: format!("User were deleted, {0}", user.username),
                response: Some(IdResponse { id: id }),
            }),
            StatusCode::CONFLICT,
        )),
        Ok(op) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: format!("Unexpected operation status, {} {}", user.username, op),
                response: None,
            }),
            StatusCode::BAD_REQUEST,
        )),
        Err(e) => {
            log::error!("Failed to register user: {}", e);
            Ok(warp::reply::with_status(
                warp::reply::json(&ResponseMessage::<Option<uuid::Uuid>> {
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    message: format!("Internal error: {}", e),
                    response: None,
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

// PUT /user
pub async fn put_user_handler<N, C>(
    user: UserIdQueryParam,
    user_req: UserUpdateReq,
    state: SyncState<N, C>,
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
    Connection: From<C>,
{
    log::debug!("Received user update request: {:?}", user_req);

    let user_id = user.user_id;

    match SyncOp::update_user(&state, &user_id, user_req).await {
        Ok(StorageOperationStatus::Updated(id)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage {
                status: StatusCode::OK.as_u16(),
                message: format!("User updated with id {}", user_id),
                response: Some(IdResponse { id: id }),
            }),
            StatusCode::OK,
        )),
        Ok(StorageOperationStatus::NotFound(id)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::NOT_FOUND.as_u16(),
                message: format!("User Not Found, {0}", id),
                response: None,
            }),
            StatusCode::NOT_FOUND,
        )),
        Ok(StorageOperationStatus::NotModified(id)) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::NOT_MODIFIED.as_u16(),
                message: format!("User Not Modified, {0}", user_id),
                response: Some(IdResponse { id: id }),
            }),
            StatusCode::NOT_MODIFIED,
        )),
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: format!("Unexpected operation status, {}", user_id),
                response: None,
            }),
            StatusCode::BAD_REQUEST,
        )),
        Err(e) => {
            log::error!("Failed to update user: {}", e);
            Ok(warp::reply::with_status(
                warp::reply::json(&ResponseMessage::<Option<uuid::Uuid>> {
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    message: format!("Internal error: {}", e),
                    response: None,
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

// GET /user?user_id=<>
pub async fn get_user_handler<N, C>(
    user_req: UserIdQueryParam,
    state: SyncState<N, C>,
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

    let mem = state.memory.lock().await;

    match mem.users.get(&user_req.user_id) {
        Some(user) if !user.is_deleted => {
            let response = ResponseMessage {
                status: 200,
                message: "User Data".to_string(),
                response: Some(user),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        _ => {
            let response = ResponseMessage::<
                Option<Vec<(uuid::Uuid, ConnectionStat, Tag, ConnectionStatus)>>,
            > {
                status: 404,
                message: "User not found".to_string(),
                response: None,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ))
        }
    }
}

// GET /users
pub async fn get_users_handler<N, C>(
    state: SyncState<N, C>,
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
    let mem = state.memory.lock().await;
    let users: Vec<(&uuid::Uuid, &User)> = mem.users.iter().collect();

    let response = ResponseMessage::<Option<Vec<(&uuid::Uuid, &User)>>> {
        status: 200,
        message: "List of users".to_string(),
        response: Some(users),
    };
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

// GET /user/stat?user_id=<>
pub async fn user_conn_stat_handler<N, C>(
    user_req: UserIdQueryParam,
    state: SyncState<N, C>,
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

    let mem = state.memory.lock().await;
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

// DELETE /user?user_id=<>
pub async fn delete_user_handler<N, C>(
    user_req: UserIdQueryParam,
    publisher: ZmqPublisher,
    state: SyncState<N, C>,
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
    Connection: From<C>,
{
    log::debug!("Run delete_user_handler");

    let (user, conns) = {
        let mem = state.memory.lock().await;

        let Some(user) = mem.users.get(&user_req.user_id) else {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::NOT_FOUND.as_u16(),
                message: "User not found".to_string(),
                response: None,
            };

            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ));
        };

        let conns = mem.connections.get_by_user_id(&user_req.user_id);

        (user.clone(), conns)
    };

    match SyncOp::delete_user(&state, &user_req.user_id).await {
        Ok(StorageOperationStatus::Ok(id)) => {
            if let Some(conns) = conns {
                for (conn_id, conn) in conns {
                    let tag = conn.get_proto().proto();

                    let msg = Message {
                        action: Action::Delete,
                        conn_id,
                        password: user.password.clone(),
                        tag: tag,
                        wg: None,
                    };
                    if let Err(e) = SyncOp::delete_connection(&state, &conn_id).await {
                        log::error!("Cannot delete connection {}", e);
                    }
                    let _ = publisher.send(&conn.get_env(), msg.clone()).await;
                }
            }
            let response = ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::OK.as_u16(),
                message: format!("User deleted {id}"),
                response: Some(IdResponse { id: id }),
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                warp::http::StatusCode::OK,
            ));
        }
        Ok(StorageOperationStatus::NotFound(_id)) => {
            let response = ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::NOT_FOUND.as_u16(),
                message: format!("User not found"),
                response: None,
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                warp::http::StatusCode::NOT_FOUND,
            ));
        }
        Err(e) => {
            log::warn!("Failed delete user {} ", user_req.user_id);
            let response = ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                message: format!("Internal error {}", e),
                response: None,
            };

            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ));
        }
        _ => unreachable!(),
    }
}
