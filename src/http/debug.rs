use core::fmt;
use futures::{SinkExt, StreamExt};
use log::info;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use warp::ws::Message;
use warp::Filter;

use crate::state::state::State;

enum Kind {
    User,
    Users,
    Nodes,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::User => write!(f, "user"),
            Kind::Users => write!(f, "users"),
            Kind::Nodes => write!(f, "nodes"),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Request {
    pub kind: String,
    pub message: String,
    pub user_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    pub kind: String,
    pub data: serde_json::Value,
    pub len: usize,
}

pub async fn start_ws_server(state: Arc<Mutex<State>>, ipaddr: Ipv4Addr, port: u16) {
    let health_check = warp::path("health-check").map(|| format!("Server OK"));

    let ws = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let state = state.clone();
            info!("Upgrading connection to websocket");
            ws.on_upgrade(move |socket| handle_debug_connection(socket, state))
        });

    info!("Server is running on ws://{}:{}", ipaddr, port);

    let routes = health_check.or(ws).with(warp::cors().allow_any_origin());
    warp::serve(routes)
        .run(SocketAddr::new(IpAddr::V4(ipaddr), port))
        .await;
}

pub async fn handle_debug_connection(socket: warp::ws::WebSocket, state: Arc<Mutex<State>>) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        let message = match msg.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let req: Request = serde_json::from_str(message).unwrap();

        if req.kind == "get_users" {
            let state = state.lock().await;
            let users: Vec<_> = state.users.keys().collect();
            let data = serde_json::to_string(&*users).unwrap();
            let response = Response {
                kind: Kind::Users.to_string(),
                data: serde_json::Value::String(data),
                len: state.users.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        } else if req.kind == "get_nodes" {
            let state = state.lock().await;
            let nodes: Vec<_> = state
                .nodes
                .values()
                .flat_map(|v| v.iter())
                .cloned()
                .collect();

            let data = serde_json::to_string(&*nodes).unwrap();
            let response = Response {
                kind: Kind::Nodes.to_string(),
                data: serde_json::Value::String(data),
                len: state.users.len(),
            };
            let response_str = serde_json::to_string(&response).unwrap();
            sender.send(Message::text(response_str)).await.unwrap();
        } else if req.kind == "get_user_info" {
            if let Some(user_id) = req.user_id {
                let state = state.lock().await;
                if let Some(user) = state.users.get(&user_id) {
                    let response = Response {
                        kind: Kind::User.to_string(),
                        data: serde_json::Value::String(user.to_string()),
                        len: 1,
                    };
                    let response_str = serde_json::to_string(&response).unwrap();

                    sender.send(Message::text(response_str)).await.unwrap();
                }
            }
        }
    }
}
