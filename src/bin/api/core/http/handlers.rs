use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{http::StatusCode, Rejection, Reply};

use pony::http::requests::ConnRequest;
use pony::http::requests::NodeRequest;
use pony::http::requests::NodeResponse;
use pony::http::requests::NodesQueryParams;
use pony::http::requests::UserQueryParam;
use pony::http::AuthError;
use pony::http::MethodError;
use pony::http::ResponseMessage;
use pony::postgres::PgContext;
use pony::state::connection::Conn;
use pony::state::connection::ConnApiOp;
use pony::state::connection::ConnBaseOp;
use pony::state::node::NodeStatus;
use pony::state::state::ConnStorageApi;
use pony::state::state::NodeStorage;
use pony::state::state::State;
use pony::utils;
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::zmq::publisher::Publisher as ZmqPublisher;

use super::JsonError;

/// Handler creates connection
pub async fn create_connection_handler<T, C>(
    conn_req: ConnRequest,
    publisher: ZmqPublisher,
    state: Arc<Mutex<State<T, C>>>,
    limit: i64,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let message = conn_req.as_message();
    log::debug!("Message sent {}", message);
    match publisher.send(&conn_req.env, message).await {
        Ok(_) => {
            let trial = conn_req.trial.unwrap_or(true);
            let limit = conn_req.limit.unwrap_or(limit);

            let mut state = state.lock().await;
            let conn: C = Conn::new(trial, limit, conn_req.env, conn_req.password).into();

            let _ = state.connections.add_or_update(&conn_req.conn_id, conn);

            let response = ResponseMessage {
                status: 200,
                message: "Connection Created".to_string(),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Err(err) => {
            let error_message = format!("Error: Cannot handle /conn req: {}", err);
            let json_error_message = warp::reply::json(&error_message);
            Ok(warp::reply::with_status(
                json_error_message,
                StatusCode::BAD_REQUEST,
            ))
        }
    }
}

pub async fn get_nodes_handler<T, C>(
    node_req: NodesQueryParams,
    state: Arc<Mutex<State<T, C>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static,
{
    let state = state.lock().await;

    match state.nodes.by_env(&node_req.env) {
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

pub async fn node_register<T, C>(
    node_req: NodeRequest,
    state: Arc<Mutex<State<T, C>>>,
    db: PgContext,
    publisher: ZmqPublisher,
) -> Result<impl Reply, Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static,
{
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let mut state = state.lock().await;

    let was_added = state.nodes.add(node.clone()).is_ok();
    let _ = db.node().insert(node.clone()).await;
    let _ = db
        .node()
        .update_status(&node.uuid, &node.env, NodeStatus::Online)
        .await;
    if let Some(node_mut) = state.nodes.get_mut(&node.env, &node.uuid) {
        let _ = node_mut.update_status(NodeStatus::Online);
    }

    if let Ok(conns_map) = db.conn().by_env(&node.env).await {
        let send_task = async {
            for conn in conns_map {
                let message = Message {
                    conn_id: conn.conn_id,
                    action: Action::Create,
                    password: Some(conn.password.clone()),
                };

                if let Err(e) = publisher.send(&node.uuid.to_string(), message).await {
                    log::error!("Failed to send init conn {}: {}", conn.conn_id, e);
                }
            }
        };
        utils::measure_time(send_task, format!("Init node {}", node.hostname)).await;
    }

    let response = if was_added {
        ResponseMessage::<String> {
            status: 200,
            message: format!("node {} is added", node_req.uuid),
        }
    } else {
        ResponseMessage::<String> {
            status: 304,
            message: format!("node {} is already known", node_req.uuid),
        }
    };

    log::debug!("Sending JSON response: {:?}", response);

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

//pub async fn connections_lines_handler<T, C>(
//    user_req: UserQueryParam,
//    state: Arc<Mutex<State<T, C>>>,
//    publisher: ZmqPublisher,
//    db: PgContext,
//) -> Result<impl warp::Reply, warp::Rejection>
//where
//    T: NodeStorage + Sync + Send + Clone + 'static,
//    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static,
//{
//    let state = state.lock().await;
//
//    let username = user_req.username;
//
//    if let Some(user) = db.user().get(&username).await {
//        if let Some(user_id) = state.users.get(&user.user_id) {
//            if let Some(connections) = state.connections.get_by_user_id(&user_id) {}
//        }
//    }
//
//    let conn = state.connections.get(&conn_id);
//    if let Some(conn) = conn {
//        if let Some(nodes) = state.nodes.by_env(env) {
//            let connections: Vec<String> = nodes
//                .iter()
//                .filter(|node| node.status == NodeStatus::Online)
//                .flat_map(|node| {
//                    node.inbounds.iter().filter_map(|(tag, inbound)| match tag {
//                        Tag::VlessXtls => utils::vless_xtls_conn(
//                            &conn_id,
//                            node.address,
//                            inbound.clone(),
//                            node.label.clone(),
//                        ),
//                        Tag::VlessGrpc => utils::vless_grpc_conn(
//                            &conn_id,
//                            node.address,
//                            inbound.clone(),
//                            node.label.clone(),
//                        ),
//                        Tag::Vmess => {
//                            utils::vmess_tcp_conn(&conn_id, node.address, inbound.clone())
//                        }
//                        _ => None,
//                    })
//                })
//                .collect();
//
//            return Ok(warp::reply::json(&connections));
//        } else {
//            return Ok(warp::reply::json(
//                &serde_json::json!({ "error": "NODES NOT FOUND" }),
//            ));
//        }
//    }
//
//    Err(warp::reject::not_found())
//}

pub async fn user_register<T, C>(
    user_req: UserQueryParam,
    state: Arc<Mutex<State<T, C>>>,
    db: PgContext,
) -> Result<impl Reply, Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static,
{
    log::debug!("Received: {:?}", user_req);

    let user_id = uuid::Uuid::new_v4();

    match db.user().insert(&user_id, &user_req.username).await {
        Ok(_) => {
            {
                let mut state = state.lock().await;
                state.users.insert(user_id);
            }

            let response = ResponseMessage::<String> {
                status: 200,
                message: format!("User {} is registered", user_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Err(err) => {
            log::error!("Failed to insert user into DB: {}", err);

            let response = ResponseMessage::<String> {
                status: 500,
                message: format!("User {} is not registered", user_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub async fn rejection(reject: Rejection) -> Result<impl Reply, Rejection> {
    if reject.find::<MethodError>().is_some() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": "Method Not Allowed"
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::METHOD_NOT_ALLOWED,
        ))
    } else if let Some(_) = reject.find::<AuthError>() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": "UNAUTHORIZED"
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::UNAUTHORIZED,
        ))
    } else if let Some(err) = reject.find::<JsonError>() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": err.0
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::BAD_REQUEST,
        ))
    } else if reject.is_not_found() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": "Not Found"
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::NOT_FOUND,
        ))
    } else {
        Err(reject)
    }
}
