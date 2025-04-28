use std::sync::Arc;
use tokio::sync::Mutex;
use zmq::Socket as ZmqSocket;

use super::Topic;

pub struct Subscriber {
    socket: Arc<Mutex<ZmqSocket>>,
    pub topics: Vec<String>,
}

impl Subscriber {
    pub fn new(endpoint: &str, uuid: &uuid::Uuid, env: &str) -> Self {
        let context = zmq::Context::new();
        let socket = context
            .socket(zmq::SUB)
            .expect("Failed to create SUB socket");

        socket
            .connect(endpoint)
            .expect("Failed to connect SUB socket");

        let topics = vec![format!("{}", *uuid), format!("{}", env)];
        log::info!("Subscribed to topics: {:?}", Topic::all(uuid, env));

        for topic in &topics {
            socket
                .set_subscribe(topic.as_bytes())
                .expect("Failed to subscribe to topic");
        }

        Self {
            socket: Arc::new(Mutex::new(socket)),
            topics: topics,
        }
    }

    pub fn clone(&self) -> Self {
        Self {
            socket: Arc::clone(&self.socket),
            topics: self.topics.clone(),
        }
    }

    pub async fn recv(&self) -> Option<String> {
        let socket = self.socket.clone();
        tokio::task::spawn_blocking(move || {
            let socket = socket.blocking_lock();
            match socket.recv_string(0) {
                Ok(Ok(data)) => {
                    log::debug!("🔵 Received from ZMQ socket: {}", data);
                    Some(data)
                }
                Ok(Err(e)) => {
                    log::error!("🔴 Failed to decode string from ZMQ: {:?}", e);
                    None
                }
                Err(e) => {
                    log::error!("🔴 Failed to receive from ZMQ: {}", e);
                    None
                }
            }
        })
        .await
        .ok()
        .flatten()
    }
}
