use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use zmq;
use zmq::Socket;

use crate::{Error, Topic};

#[derive(Clone)]
pub struct Publisher {
    socket: Arc<Mutex<Socket>>,
}

impl Publisher {
    pub async fn bind(endpoint: &str) -> Result<Self, Error> {
        Self::init(endpoint, true).await
    }

    pub async fn connect(endpoint: &str) -> Result<Self, Error> {
        Self::init(endpoint, false).await
    }

    async fn init(endpoint: &str, is_bind: bool) -> Result<Self, Error> {
        let context = zmq::Context::new();
        let publisher = context.socket(zmq::PUB).expect("Failed to create socket");

        publisher.set_sndhwm(1000)?;

        let mut i = 0;
        loop {
            let result = if is_bind {
                publisher.bind(endpoint)
            } else {
                publisher.connect(endpoint)
            };

            match result {
                Ok(_) => {
                    let mode = if is_bind { "Bound to" } else { "Connected to" };
                    tracing::debug!("PUB: {} {}", mode, endpoint);
                    break;
                }
                Err(err) => {
                    if i >= 5 {
                        panic!(
                            "Failed to setup PUB socket at {}: {}. Giving up.",
                            endpoint, err
                        );
                    }
                    tracing::warn!("PUB: Setup attempt {} failed: {}", i + 1, err);
                    i += 1;
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        sleep(Duration::from_millis(1000)).await;

        Ok(Self {
            socket: Arc::new(Mutex::new(publisher)),
        })
    }

    pub async fn new(endpoint: &str) -> Result<Self, Error> {
        Self::bind(endpoint).await
    }

    pub async fn send_binary(&self, topic: &Topic, payload: &[u8]) -> zmq::Result<()> {
        let socket = self.socket.lock().await;

        socket.send(topic.as_str().as_ref(), zmq::SNDMORE)?;
        socket.send(payload, 0)?;

        tracing::debug!("PUB: Message sent: {} | {} bytes", topic, payload.len());
        Ok(())
    }
}
