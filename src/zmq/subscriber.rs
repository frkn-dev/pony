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

        let topics = vec![
            Topic::Init(uuid.to_string()).as_zmq_topic(),
            Topic::Updates(env.to_string()).as_zmq_topic(),
            Topic::All.as_zmq_topic(),
        ];
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

    pub async fn recv(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        let socket = self.socket.clone();

        let result = tokio::task::spawn_blocking(move || {
            let socket = socket.blocking_lock();

            let topic = match socket.recv_bytes(0) {
                Ok(t) => t,
                Err(e) => {
                    log::error!("ZMQ recv topic failed: {}", e);
                    return None;
                }
            };

            let payload = match socket.recv_bytes(0) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("ZMQ recv payload failed: {}", e);
                    return None;
                }
            };

            Some((topic, payload))
        })
        .await;

        match result {
            Ok(Some(pair)) => Some(pair),
            Ok(None) => {
                log::warn!("ZMQ multipart recv returned None");
                None
            }
            Err(e) => {
                log::error!("ZMQ recv_multipart join error: {}", e);
                None
            }
        }
    }
}
