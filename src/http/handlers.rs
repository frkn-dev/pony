use std::sync::Arc;

use log::debug;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;
use warp::{http::StatusCode, reject, Rejection, Reply};

use crate::http::JsonError;
use crate::measure_time;
use crate::postgres::DbContext;
use crate::state::state::NodeStorage;
use crate::state::state::UserStorage;
use crate::state::{
    node::NodeStatus,
    node::{NodeRequest, NodeResponse},
    state::State,
    tag::Tag,
    user::User,
};
use crate::vless_grpc_conn;
use crate::vless_xtls_conn;
use crate::vmess_tcp_conn;
use crate::ZmqPublisher;
use crate::{Action, Message};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserRequest {
    pub user_id: Uuid,
    pub action: Action,
    pub env: String,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub password: Option<String>,
}

impl UserRequest {
    pub fn as_message(&self, limit: i64) -> Message {
        Message {
            user_id: self.user_id,
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

#[derive(Serialize, Debug, Deserialize)]
pub struct ResponseMessage<T> {
    pub status: u16,
    pub message: T,
}

pub async fn user_request<T>(
    user_req: UserRequest,
    publisher: ZmqPublisher,
    state: Arc<Mutex<State<T>>>,
    limit: i64,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let message = user_req.as_message(limit);
    println!("{}", message);
    match publisher.send(&user_req.env, message).await {
        Ok(_) => {
            let trial = user_req.trial.unwrap_or(true);
            let limit = user_req.limit.unwrap_or(limit);

            let mut state = state.lock().await;
            let user = User::new(trial, limit, user_req.env, user_req.password);
            let _ = state.add_or_update_user(user_req.user_id, user);

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

pub async fn create_user_from_tg<T>(username: String) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let message = user_req.as_message(limit);
    println!("{}", message);
    match publisher.send(&user_req.env, message).await {
        Ok(_) => {
            let trial = user_req.trial.unwrap_or(true);
            let limit = user_req.limit.unwrap_or(limit);

            let mut state = state.lock().await;
            let user = User::new(trial, limit, user_req.env, user_req.password);
            let _ = state.add_or_update_user(user_req.user_id, user);

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

pub async fn get_nodes<T>(
    node_req: NodesQueryParams,
    state: Arc<Mutex<State<T>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let state = state.lock().await;

    match state.nodes.get_nodes(node_req.env) {
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
    db: DbContext,
    publisher: ZmqPublisher,
) -> Result<impl Reply, Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let mut state = state.lock().await;

    let was_added = state.nodes.add_node(node.clone()).is_ok();
    let _ = db.node().insert_node(node.clone()).await;
    let _ = db
        .node()
        .update_node_status(node.uuid, &node.env, NodeStatus::Online)
        .await;
    if let Some(node_mut) = state.nodes.get_mut_node(&node.env, node.uuid) {
        let _ = node_mut.update_status(NodeStatus::Online);
    }

    if let Ok(users_map) = db.user().get_users_by_cluster(&node.env).await {
        let send_task = async {
            for user in users_map {
                let message = Message {
                    user_id: user.user_id,
                    action: Action::Create,
                    env: node.env.clone(),
                    trial: user.trial,
                    limit: user.limit,
                    password: Some(user.password.clone()),
                };

                if let Err(e) = publisher.send(&node.uuid.to_string(), message).await {
                    log::error!("Failed to send init user {}: {}", user.user_id, e);
                }
            }
        };
        measure_time(send_task, format!("Init node {}", node.hostname)).await;
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

    debug!("Sending JSON response: {:?}", response);

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

#[derive(Serialize, Deserialize)]
pub struct UserQueryParams {
    pub id: Uuid,
}

pub async fn get_conn<T>(
    user_req: UserQueryParams,
    state: Arc<Mutex<State<T>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let state = state.lock().await;

    let user_id = user_req.id;

    let user = state.get_user(user_id);
    if let Some(user) = user {
        let env = user.env;

        if let Some(nodes) = state.nodes.get_nodes(env) {
            let connections: Vec<String> = nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Online)
                .flat_map(|node| {
                    debug!("TAGS {:?}", node.inbounds.keys());
                    node.inbounds.iter().filter_map(|(tag, inbound)| match tag {
                        Tag::VlessXtls => {
                            vless_xtls_conn(user_id, node.ipv4, inbound.clone(), node.label.clone())
                        }
                        Tag::VlessGrpc => {
                            vless_grpc_conn(user_id, node.ipv4, inbound.clone(), node.label.clone())
                        }
                        Tag::Vmess => vmess_tcp_conn(user_id, node.ipv4, inbound.clone()),
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
