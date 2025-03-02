use base64::{engine::general_purpose, Engine as _};
use log::debug;
use serde::Deserialize;
use serde::Serialize;
use serde_json::to_string;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;
use warp::http::StatusCode;
use warp::reject;
use warp::Rejection;
use warp::Reply;
use zmq::Socket;

use crate::config::xray::Inbound;
use crate::state::{
    node::Node,
    node::NodeStatus,
    node::{NodeRequest, NodeResponse},
    state::State,
    tag::Tag,
    user::User,
};
use crate::zmq::{message::Message, publisher};

pub type UserRequest = Message;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}

#[derive(Debug)]
pub struct AuthError(pub String);
impl warp::reject::Reject for AuthError {}

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
            let trial = match user_req.trial {
                Some(trial) => trial,
                None => {
                    return Err(warp::reject::custom(JsonError(
                        "Missing 'trial' value".to_string(),
                    )))
                }
            };

            let limit = match user_req.limit {
                Some(limit) => limit,
                None => {
                    return Err(warp::reject::custom(JsonError(
                        "Missing 'limit' value".to_string(),
                    )))
                }
            };

            let mut state = state.lock().await;
            let user = User::new(trial, limit, user_req.env, user_req.password);
            let _ = state.add_user(user_req.user_id, user).await;

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
    client: Arc<Mutex<Client>>,
    _publisher: Arc<Mutex<Socket>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let mut state = state.lock().await;

    if state.add_node(node.clone()).await.is_ok() {
        let _ = Node::insert_node(client, node).await;
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

fn vless_xtls_conn(
    user_id: Uuid,
    ipv4: Ipv4Addr,
    inbound: Inbound,
    label: String,
) -> Option<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings?;
    let reality_settings = stream_settings.reality_settings?;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first()?;
    let sni = reality_settings.server_names.first()?;

    let conn = format!(
        "vless://{user_id}@{ipv4}:{port}?security=reality&flow=xtls-rprx-vision&type=tcp&sni={sni}&fp=chrome&pbk={pbk}&sid={sid}#{label} XTLS"
    );
    debug!("Conn XTLS Vless - {}", conn);

    Some(conn)
}

fn vless_grpc_conn(
    user_id: Uuid,
    ipv4: Ipv4Addr,
    inbound: Inbound,
    label: String,
) -> Option<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings?;
    let reality_settings = stream_settings.reality_settings?;
    let grpc_settings = stream_settings.grpc_settings?;
    let service_name = grpc_settings.service_name;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first()?;
    let sni = reality_settings.server_names.first()?;

    let conn = format!(
        "vless://{user_id}@{ipv4}:{port}?security=reality&type=grpc&mode=gun&serviceName={service_name}&fp=chrome&sni={sni}&pbk={pbk}&sid={sid}#{label} GRPC"
    );
    debug!("Conn GRPC Vless - {}", conn);

    Some(conn)
}

pub fn vmess_tcp_conn(
    user_id: Uuid,
    ipv4: Ipv4Addr,
    inbound: Inbound,
    label: String,
) -> Option<String> {
    let mut conn: HashMap<String, String> = HashMap::new();
    let port = inbound.port;
    let stream_settings = inbound.stream_settings?;
    let tcp_settings = stream_settings.tcp_settings?;
    let header = tcp_settings.header?;
    let req = header.request?;
    let headers = req.headers?;

    let host = headers.get("Host")?.first()?;
    let path = req.path.first()?;

    conn.insert("add".to_string(), ipv4.to_string());
    conn.insert("aid".to_string(), "0".to_string());
    conn.insert("host".to_string(), host.to_string());
    conn.insert("id".to_string(), user_id.to_string());
    conn.insert("net".to_string(), "tcp".to_string());
    conn.insert("path".to_string(), path.to_string());
    conn.insert("port".to_string(), port.to_string());
    conn.insert("ps".to_string(), "VmessTCP".to_string());
    conn.insert("scy".to_string(), "auto".to_string());
    conn.insert("tls".to_string(), "none".to_string());
    conn.insert("type".to_string(), "http".to_string());
    conn.insert("v".to_string(), "2".to_string());

    let json_str = to_string(&conn).ok()?;

    debug!("JSON STR {:?}", json_str);

    let base64_str = general_purpose::STANDARD.encode(json_str);

    Some(format!("vmess://{base64_str}#{label}"))
}

#[derive(Serialize, Deserialize)]
pub struct UserQueryParams {
    pub id: Uuid,
}

pub async fn get_conn(
    user_req: UserQueryParams,
    state: Arc<Mutex<State>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = state.lock().await;

    let user_id = user_req.id;

    let user = state.get_user(user_id).await;
    if let Some(user) = user {
        let env = user.env;

        if let Some(nodes) = state.get_nodes(env) {
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
                        Tag::Vmess => {
                            vmess_tcp_conn(user_id, node.ipv4, inbound.clone(), node.label.clone())
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
