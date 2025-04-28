use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use core::fmt;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use warp::ws::Message;
use warp::Filter;

use crate::state::state::NodeStorage;
use crate::state::state::State;

enum Kind {
    Conn,
    Conns,
    Nodes,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Conn => write!(f, "conn"),
            Kind::Conns => write!(f, "conns"),
            Kind::Nodes => write!(f, "nodes"),
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

pub async fn start_ws_server<T>(state: Arc<Mutex<State<T>>>, ipaddr: Ipv4Addr, port: u16)
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let health_check = warp::path("health-check").map(|| format!("Server OK"));

    let ws = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let state = state.clone();
            log::info!("Upgrading connection to websocket");
            ws.on_upgrade(move |socket| handle_debug_connection(socket, state))
        });

    log::info!("Debug Server is running on ws://{}:{}", ipaddr, port);

    let routes = health_check.or(ws).with(warp::cors().allow_any_origin());
    warp::serve(routes)
        .run(SocketAddr::new(IpAddr::V4(ipaddr), port))
        .await;
}

pub async fn handle_debug_connection<T>(socket: warp::ws::WebSocket, state: Arc<Mutex<State<T>>>)
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        let message = match msg.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let req: Request = serde_json::from_str(message).unwrap();

        if req.kind == "get_connections" {
            let state = state.lock().await;
            let conns: Vec<_> = state.connections.keys().collect();
            let data = serde_json::to_string(&*conns).unwrap();
            let response = Response {
                kind: Kind::Conns.to_string(),
                data: serde_json::Value::String(data),
                len: state.connections.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        } else if req.kind == "get_nodes" {
            let state = state.lock().await;
            let nodes = state.nodes.all_json();

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
                len: state.connections.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        } else if req.kind == "get_conn_info" {
            if let Some(conn_id) = req.conn_id {
                let state = state.lock().await;
                if let Some(conn) = state.connections.get(&conn_id) {
                    let response = Response {
                        kind: Kind::Conn.to_string(),
                        data: serde_json::Value::String(conn.to_string()),
                        len: 1,
                    };
                    let response_str = serde_json::to_string(&response).unwrap();

                    sender.send(Message::text(response_str)).await.unwrap();
                }
            }
        }
    }
}
