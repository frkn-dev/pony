use crate::node::NodeRequest;
use crate::node::NodeResponse;
use crate::user::User;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::http::StatusCode;
use warp::reject;
use warp::Rejection;
use warp::Reply;
use zmq::Socket;

use crate::message::Message;
use crate::state::State;
use crate::zmq::publisher;

pub type UserRequest = Message;

#[derive(Debug)]
struct JsonError(String);

impl reject::Reject for JsonError {}

#[derive(Debug)]
struct MethodError;
impl reject::Reject for MethodError {}

#[derive(Serialize)]
struct ResponseMessage<T> {
    status: u16,
    message: T,
}

pub async fn user_request(
    user_req: UserRequest,
    publisher: Arc<Mutex<Socket>>,
    state: Arc<Mutex<State>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match publisher::send_message(publisher, &user_req.env, user_req.clone()).await {
        Ok(_) => {
            let mut state = state.lock().await;
            let user = User::new(
                user_req.trial.expect("REASON"),
                user_req.limit.expect("REASON"),
                user_req.password,
            );
            let _ = state.add_user(user_req.user_id, user, true).await;

            let response = ResponseMessage {
                status: 200,
                message: "User Created".to_string(),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Err(err) => {
            let error_message = format!("Error: Cannot handle /user req: {}", err);
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

pub async fn get_nodes(
    node_req: NodesQueryParams,
    state: Arc<Mutex<State>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = state.lock().await;

    match state.get_nodes(node_req.env) {
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

pub async fn node_register(
    node_req: NodeRequest,
    state: Arc<Mutex<State>>,
    _publisher: Arc<Mutex<Socket>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let mut state = state.lock().await;

    if state.add_node(node).await.is_ok() {
        let response = ResponseMessage::<String> {
            status: 200,
            message: format!("node {} is added", node_req.uuid),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 304,
            message: format!("node {} is not added", node_req.uuid),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_MODIFIED,
        ))
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
