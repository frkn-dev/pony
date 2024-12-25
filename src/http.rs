use futures::{SinkExt, StreamExt};
use log::info;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use warp::filters::ws::Message;
use warp::Filter;

use crate::state::State;

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

pub async fn start_ws_server(state: Arc<Mutex<State>>) {
    let health_check = warp::path("health-check").map(|| format!("Server OK"));

    let ws = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let state = state.clone();
            info!("Upgrading connection to websocket");
            ws.on_upgrade(move |socket| handle_connection(socket, state))
        });

    let routes = health_check.or(ws).with(warp::cors().allow_any_origin());

    warp::serve(routes)
        .run(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            3000,
        ))
        .await;
    info!("Server is running on ws://127.0.0.1:3000");
}

async fn handle_connection(socket: warp::ws::WebSocket, state: Arc<Mutex<State>>) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        let message = match msg.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let req: Request = serde_json::from_str(message).unwrap();

        if req.kind == "get_state" {
            let state = state.lock().await;
            let users: Vec<_> = state.users.keys().collect();
            let data = serde_json::to_string(&*users).unwrap();
            let response = Response {
                kind: "state".to_string(),
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
                        kind: "user".to_string(),
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
