use warp::http::StatusCode;

use pony::http::requests::ConnQueryParam;
use pony::http::requests::ConnRequest;
use pony::http::requests::NodeRequest;
use pony::http::requests::NodeResponse;
use pony::http::requests::NodesQueryParams;
use pony::http::requests::UserIdQueryParam;
use pony::http::requests::UserRegQueryParam;
use pony::http::ResponseMessage;
use pony::state::Conn;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::ConnStat;
use pony::state::ConnStatus;
use pony::state::ConnStorageApi;
use pony::state::ConnStorageBase;
use pony::state::ConnStorageOpStatus;
use pony::state::NodeStatus;
use pony::state::NodeStorage;
use pony::state::NodeStorageOpStatus;
use pony::state::SyncOp;
use pony::state::SyncState;
use pony::state::Tag;
use pony::state::User;
use pony::state::UserStorage;
use pony::state::UserStorageOpStatus;

use pony::utils;
use pony::zmq::message::Action;

use pony::zmq::message::Message;
use pony::zmq::publisher::Publisher as ZmqPublisher;

pub async fn healthcheck<T, C>(state: SyncState<T, C>) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let mem = state.memory.lock().await;
    let users = mem.users.all();

    if let Ok(_users) = users {
        let response = ResponseMessage::<String> {
            status: 200,
            message: "OK".to_string(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 404,
            message: "Users not found".to_string(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}

pub async fn get_users_handler<T, C>(
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let mem = state.memory.lock().await;
    let users = mem.users.all();

    if let Ok(users) = users {
        let response = ResponseMessage::<Vec<(uuid::Uuid, User)>> {
            status: 200,
            message: users,
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 404,
            message: "Users not found".to_string(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}

pub async fn user_register_handler<T, C>(
    user_req: UserRegQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Received: {:?}", user_req);

    let user_id = uuid::Uuid::new_v4();
    let user = User::new(user_req.username);

    match SyncOp::add_user(&state, &user_id, user).await {
        Ok(UserStorageOpStatus::Ok) => {
            let response = ResponseMessage::<uuid::Uuid> {
                status: 200,
                message: user_id,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Ok(UserStorageOpStatus::AlreadyExist) => {
            let response = ResponseMessage::<String> {
                status: 304,
                message: format!("User {} is already exist", user_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_MODIFIED,
            ))
        }
        Err(e) => {
            log::error!("{}", e);
            let response = ResponseMessage::<String> {
                status: 500,
                message: format!("Error: {}", e),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub async fn user_conn_stat_handler<T, C>(
    user_req: UserIdQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Received: {:?}", user_req);

    let mem = state.memory.lock().await;

    let mut result: Vec<(uuid::Uuid, ConnStat, Tag)> = vec![];

    if let Some(connections) = mem.connections.get_by_user_id(&user_req.user_id) {
        for (conn_id, conn) in connections {
            let stat = ConnStat {
                online: conn.get_online(),
                downlink: conn.get_downlink(),
                uplink: conn.get_uplink(),
            };

            result.push((conn_id, stat, conn.get_proto()));
        }

        let response = ResponseMessage::<Vec<(uuid::Uuid, ConnStat, Tag)>> {
            status: 200,
            message: result,
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 404,
            message: "Connections not found".to_string(),
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    }
}

// Register node handler
pub async fn node_register<T, C>(
    node_req: NodeRequest,
    state: SyncState<T, C>,
    publisher: ZmqPublisher,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let node_id = node_req.uuid;

    match SyncOp::add_node(&state, &node_id, node.clone()).await {
        Ok(status @ (NodeStorageOpStatus::Ok | NodeStorageOpStatus::AlreadyExist)) => {
            if status == NodeStorageOpStatus::AlreadyExist {
                let _ = SyncOp::update_node_status(&state, &node_id, &node.env, NodeStatus::Online)
                    .await;
            }

            let mem = state.memory.lock().await;
            for (conn_id, conn) in mem.connections.iter() {
                let msg = Message {
                    conn_id: *conn_id,
                    action: Action::Create,
                    password: conn.get_password(),
                    tag: conn.get_proto(),
                };
                let _ = publisher.send(&node_req.uuid.to_string(), msg).await;
            }

            let (status_code, message) = match status {
                NodeStorageOpStatus::Ok => (StatusCode::OK, format!("node {} is added", node_id)),
                NodeStorageOpStatus::AlreadyExist => (
                    StatusCode::NOT_MODIFIED,
                    format!("node {} already exists", node_id),
                ),
            };

            let response = ResponseMessage::<String> {
                status: status_code.as_u16(),
                message,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                status_code,
            ))
        }

        Err(e) => {
            let response = ResponseMessage::<String> {
                status: 500,
                message: format!("INTERNAL_SERVER_ERROR {} for {}", e, node_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Handler creates connection
pub async fn create_or_update_connection_handler<T, C>(
    conn_req: ConnRequest,
    publisher: ZmqPublisher,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let conn = Conn::new(
        conn_req.trial.unwrap_or(true),
        conn_req.limit.unwrap_or(0),
        &conn_req.env,
        ConnStatus::Active,
        conn_req.password.clone(),
        conn_req.user_id,
        ConnStat::default(),
        conn_req.proto,
    )
    .into();

    let msg = conn_req.as_message();

    match SyncOp::add_or_update_conn(&state, &conn_req.conn_id, conn).await {
        Ok(ConnStorageOpStatus::Ok) => {
            log::debug!("/connection req Message sent {}", msg);
            let _ = publisher.send(&conn_req.env.to_string(), msg.clone()).await;
            let response = ResponseMessage {
                status: 200,
                message: format!("connection {} is added", conn_req.conn_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Ok(ConnStorageOpStatus::Updated) => {
            log::debug!("/connection req Message sent {}", msg);
            let _ = publisher.send(&conn_req.env.to_string(), msg.clone()).await;
            let response = ResponseMessage {
                status: 200,
                message: format!("connection {} is modified", conn_req.conn_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Ok(ConnStorageOpStatus::AlreadyExist) => {
            let response = ResponseMessage {
                status: 304,
                message: format!("connection {} is modified", conn_req.conn_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_MODIFIED,
            ))
        }
        Ok(op) => {
            let error_message = format!("Error: Operation isn't supported: {}", op);
            let json_error_message = warp::reply::json(&error_message);
            Ok(warp::reply::with_status(
                json_error_message,
                StatusCode::BAD_REQUEST,
            ))
        }
        Err(err) => {
            let error_message = format!("Error: Cannot handle /connection req: {}", err);
            let json_error_message = warp::reply::json(&error_message);
            Ok(warp::reply::with_status(
                json_error_message,
                StatusCode::BAD_REQUEST,
            ))
        }
    }
}

/// List of nodes handler
pub async fn get_nodes_handler<T, C>(
    node_req: NodesQueryParams,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static,
{
    let state = state.memory.lock().await;

    let nodes = state.nodes.by_env(&node_req.env);

    match nodes {
        Some(nodes) => {
            let node_response: Vec<NodeResponse> = nodes
                .into_iter()
                .map(|node| node.as_node_response())
                .collect();
            let response = ResponseMessage::<Vec<NodeResponse>> {
                status: 200,
                message: node_response,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        None => {
            let response = ResponseMessage::<String> {
                status: 404,
                message: "Env not found".to_string(),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ))
        }
    }
}

/// Get user connections handler

/// Get list of user connection credentials
pub async fn get_user_connections_handler<T, C>(
    user_req: UserIdQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp
        + ConnApiOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Conn>
        + std::fmt::Debug
        + serde::ser::Serialize,
{
    log::debug!("Received: {:?}", user_req);

    let user_id = user_req.user_id;

    let connections = {
        let mem = state.memory.lock().await;
        mem.connections.get_by_user_id(&user_id)
    };

    if connections.is_none() {
        log::debug!("Connections not found");

        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "NO CONNECTIONS AVAILABLE" })),
            warp::http::StatusCode::NOT_FOUND,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&connections),
        warp::http::StatusCode::OK,
    ))
}

/// Get list of user connection credentials
pub async fn connections_lines_handler<T, C>(
    conn_req: ConnQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn> + std::fmt::Debug,
{
    let state = state.memory.lock().await;

    let conn_id = conn_req.conn_id;
    let env = conn_req.env;
    let mut connection_links = Vec::new();

    if let Some(_conn) = state.connections.get(&conn_id) {
        if let Some(nodes) = state.nodes.by_env(&env) {
            for node in nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Online)
            {
                for (tag, inbound) in &node.inbounds {
                    let link = utils::create_conn_link(
                        *tag,
                        &conn_id,
                        inbound.as_inbound_response(),
                        &node.label,
                        node.address,
                    );

                    if let Ok(link) = link {
                        connection_links.push(link);
                    }
                }
            }
        }
    }

    if connection_links.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "NO CONNECTIONS AVAILABLE" })),
            warp::http::StatusCode::NOT_FOUND,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&connection_links),
        warp::http::StatusCode::OK,
    ))
}
