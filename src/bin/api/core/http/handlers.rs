use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{http::StatusCode, reject, Rejection, Reply};

use pony::http::ResponseMessage;
use pony::http::UserRequest;
use pony::postgres::PgContext;
use pony::state::connection::Conn;
use pony::state::node::NodeRequest;
use pony::state::node::NodeResponse;
use pony::state::node::NodeStatus;
use pony::state::state::ConnStorage;
use pony::state::state::NodeStorage;
use pony::state::state::State;
use pony::state::tag::Tag;
use pony::utils;
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::zmq::publisher::Publisher as ZmqPublisher;

use super::JsonError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnRequest {
    pub conn_id: uuid::Uuid,
    pub action: Action,
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

impl ConnRequest {
    pub fn as_message(&self, limit: i64) -> Message {
        Message {
            conn_id: self.conn_id,
            action: self.action.clone(),
            env: self.env.clone(),
            trial: self.trial.unwrap_or(true),
            limit: self.limit.unwrap_or(limit),
            password: self.password.clone(),
        }
    }
}

#[derive(Debug)]
pub struct AuthError(pub String);
impl warp::reject::Reject for AuthError {}

#[derive(Debug)]
struct MethodError;
impl reject::Reject for MethodError {}

/// Handler creates connection
pub async fn create_connection_handler<T>(
    conn_req: ConnRequest,
    publisher: ZmqPublisher,
    state: Arc<Mutex<State<T>>>,
    limit: i64,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let message = conn_req.as_message(limit);
    println!("{}", message);
    match publisher.send(&conn_req.env, message).await {
        Ok(_) => {
            let trial = conn_req.trial.unwrap_or(true);
            let limit = conn_req.limit.unwrap_or(limit);

            let mut state = state.lock().await;
            let conn = Conn::new(trial, limit, conn_req.env, conn_req.password);
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

#[derive(Serialize, Deserialize)]
pub struct NodesQueryParams {
    pub env: String,
}

pub async fn get_nodes_handler<T>(
    node_req: NodesQueryParams,
    state: Arc<Mutex<State<T>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
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

pub async fn node_register<T>(
    node_req: NodeRequest,
    state: Arc<Mutex<State<T>>>,
    db: PgContext,
    publisher: ZmqPublisher,
) -> Result<impl Reply, Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
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
                    env: node.env.clone(),
                    trial: conn.trial,
                    limit: conn.limit,
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

#[derive(Serialize, Deserialize)]
pub struct ConnQueryParams {
    pub id: uuid::Uuid,
}

pub async fn connections_lines_handler<T>(
    conn_req: ConnQueryParams,
    state: Arc<Mutex<State<T>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let state = state.lock().await;

    let conn_id = conn_req.id;

    let conn = state.connections.get(&conn_id);
    if let Some(conn) = conn {
        let env = &conn.env;

        if let Some(nodes) = state.nodes.by_env(env) {
            let connections: Vec<String> = nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Online)
                .flat_map(|node| {
                    node.inbounds.iter().filter_map(|(tag, inbound)| match tag {
                        Tag::VlessXtls => utils::vless_xtls_conn(
                            &conn_id,
                            node.address,
                            inbound.clone(),
                            node.label.clone(),
                        ),
                        Tag::VlessGrpc => utils::vless_grpc_conn(
                            &conn_id,
                            node.address,
                            inbound.clone(),
                            node.label.clone(),
                        ),
                        Tag::Vmess => {
                            utils::vmess_tcp_conn(&conn_id, node.address, inbound.clone())
                        }
                        _ => None,
                    })
                })
                .collect();

            return Ok(warp::reply::json(&connections));
        } else {
            return Ok(warp::reply::json(
                &serde_json::json!({ "error": "NODES NOT FOUND" }),
            ));
        }
    }

    Err(warp::reject::not_found())
}

pub async fn user_register<T>(
    user_req: UserRequest,
    state: Arc<Mutex<State<T>>>,
    db: PgContext,
) -> Result<impl Reply, Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    log::debug!("Received: {:?}", user_req);

    let user_id = uuid::Uuid::new_v4();

    match db.user().insert(&user_id, user_req.username).await {
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
