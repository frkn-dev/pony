use core::fmt;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::http::header::SEC_WEBSOCKET_PROTOCOL;
use warp::ws::Message;
use warp::ws::Ws;
use warp::Filter;
use warp::Rejection;

use crate::http::Unauthorized;

use crate::memory::cache::Cache;
use crate::memory::connection::op::base::Operations as ConnectionBaseOp;
use crate::memory::storage::connection::BaseOp as ConnectionStorageBaseOp;
use crate::memory::storage::node::Operations as NodeStorageOp;

enum Kind {
    Conn,
    Conns,
    Nodes,
    Users,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Conn => write!(f, "conn"),
            Kind::Conns => write!(f, "conns"),
            Kind::Nodes => write!(f, "nodes"),
            Kind::Users => write!(f, "users"),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Request {
    pub kind: String,
    pub message: String,
    pub conn_id: Option<uuid::Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    pub kind: String,
    pub data: serde_json::Value,
    pub len: usize,
}

pub async fn start_ws_server<N, C>(
    memory: Arc<RwLock<Cache<N, C>>>,
    ipaddr: Ipv4Addr,
    port: u16,
    expected_token: Arc<String>,
) where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionBaseOp + Sync + Send + Clone + 'static + std::fmt::Display,
{
    let health_check = warp::path("health-check").map(|| "Server OK");

    let ws_route = {
        let expected_token = expected_token.clone();
        warp::path("ws")
            .and(warp::ws())
            .and(warp::header::optional::<String>(
                SEC_WEBSOCKET_PROTOCOL.as_str(),
            ))
            .and_then(move |ws: Ws, token: Option<String>| {
                let memory = memory.clone();
                let expected_token = expected_token.clone();
                async move {
                    match token {
                        Some(t) if t == *expected_token => {
                            Ok::<_, Rejection>(ws.on_upgrade(move |socket| {
                                handle_debug_connection::<N, C>(socket, memory)
                            }))
                        }
                        _ => {
                            log::warn!(
                                "Unauthorized WebSocket connection attempt with token: {:?}",
                                token
                            );
                            Err(warp::reject::custom(Unauthorized))
                        }
                    }
                }
            })
    };

    let routes = health_check
        .or(ws_route)
        .with(warp::cors().allow_any_origin());

    warp::serve(routes)
        .run(SocketAddr::new(IpAddr::V4(ipaddr), port))
        .await;
}

pub async fn handle_debug_connection<N, C>(
    socket: warp::ws::WebSocket,
    memory: Arc<RwLock<Cache<N, C>>>,
) where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionBaseOp + Sync + Send + Clone + 'static + std::fmt::Display,
{
    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        let message = match msg.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let req: Request = match serde_json::from_str(message) {
            Ok(r) => r,
            Err(err) => {
                log::error!("Invalid request format: {}", err);
                continue;
            }
        };

        // COMMENT(@qezz): A `match` would probably work better here.
        if req.kind == "get_connections" {
            let memory = memory.read().await;
            let conns: Vec<_> = memory.connections.keys().collect();
            let data = serde_json::to_string(&conns).unwrap();
            let response = Response {
                kind: Kind::Conns.to_string(),
                data: serde_json::Value::String(data),
                len: memory.connections.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        } else if req.kind == "get_nodes" {
            let memory = memory.read().await;
            let nodes = memory.nodes.all_json();

            let data = match serde_json::to_string(&nodes) {
                Ok(json) => json,
                Err(err) => {
                    log::error!("Failed to serialize nodes: {}", err);
                    continue;
                }
            };

            let response = Response {
                kind: Kind::Nodes.to_string(),
                data: serde_json::Value::String(data),
                len: memory.connections.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        } else if req.kind == "get_conn_info" {
            if let Some(conn_id) = req.conn_id {
                let memory = memory.read().await;
                if let Some(conn) = memory.connections.get(&conn_id) {
                    let response = Response {
                        kind: Kind::Conn.to_string(),
                        data: serde_json::Value::String(conn.to_string()),
                        len: 1,
                    };
                    let response_str = serde_json::to_string(&response).unwrap();
                    sender.send(Message::text(response_str)).await.unwrap();
                }
            }
        } else if req.kind == "get_users" {
            let memory = memory.read().await;
            let users: Vec<_> = memory.users.iter().collect();
            let data = serde_json::to_string(&users).unwrap();
            let response = Response {
                kind: Kind::Users.to_string(),
                data: serde_json::Value::String(data),
                len: users.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        }
    }
}
