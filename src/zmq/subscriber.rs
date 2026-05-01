use std::sync::Arc;
use tokio::sync::Mutex;
use zmq::Error;
use zmq::Socket as ZmqSocket;

use super::topic::Topic;

pub struct Subscriber {
    socket: Arc<Mutex<ZmqSocket>>,
    pub topics: Vec<Topic>,
}

impl Subscriber {
    pub fn new(endpoint: &str, topics: Vec<Topic>) -> Result<Self, Error> {
        let context = zmq::Context::new();
        let socket = context
            .socket(zmq::SUB)
            .expect("Failed to create SUB socket");

        tracing::debug!("Connecting SUB {} {:?}", endpoint, topics);
        socket
            .connect(endpoint)
            .expect("Failed to connect SUB socket");

        tracing::debug!("Subscribed to topics: {:?}", topics);

        for topic in &topics {
            socket
                .set_subscribe(&topic.as_bytes())
                .expect("Failed to subscribe to topic");
        }

        Ok(Self {
            socket: Arc::new(Mutex::new(socket)),
            topics,
        })
    }

    pub async fn recv(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        let socket = self.socket.clone();

        let result = tokio::task::spawn_blocking(move || {
            let socket = socket.blocking_lock();

            let topic = match socket.recv_bytes(0) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("ZMQ recv topic failed: {}", e);
                    return None;
                }
            };

            let payload = match socket.recv_bytes(0) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("ZMQ recv payload failed: {}", e);
                    return None;
                }
            };

            Some((topic, payload))
        })
        .await;

        match result {
            Ok(Some(pair)) => Some(pair),
            Ok(None) => {
                tracing::warn!("ZMQ multipart recv returned None");
                None
            }
            Err(e) => {
                tracing::error!("ZMQ recv_multipart join error: {}", e);
                None
            }
        }
    }

    pub fn new_bound(endpoint: &str, topics: Vec<Topic>) -> Result<Self, Error> {
        let context = zmq::Context::new();
        let socket = context
            .socket(zmq::SUB)
            .expect("Failed to create SUB socket");

        socket.bind(endpoint).expect("Failed to bind SUB socket");
        socket.set_rcvhwm(5000)?;

        for topic in &topics {
            socket
                .set_subscribe(&topic.as_bytes())
                .expect("Failed to subscribe to topic");
        }
        tracing::debug!("Subscribed to topics: {:?}", topics);

        Ok(Self {
            socket: Arc::new(Mutex::new(socket)),
            topics,
        })
    }
}

impl Clone for Subscriber {
    fn clone(&self) -> Self {
        Self {
            socket: Arc::clone(&self.socket),
            topics: self.topics.clone(),
        }
    }
}
