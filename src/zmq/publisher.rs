use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use zmq;
use zmq::Socket;

#[derive(Clone)]
pub struct Publisher {
    socket: Arc<Mutex<Socket>>,
}

impl Publisher {
    pub async fn bind(endpoint: &str) -> Self {
        Self::init(endpoint, true).await
    }

    pub async fn connect(endpoint: &str) -> Self {
        Self::init(endpoint, false).await
    }

    async fn init(endpoint: &str, is_bind: bool) -> Self {
        let context = zmq::Context::new();
        let publisher = context.socket(zmq::PUB).expect("Failed to create socket");

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
                    log::debug!("PUB: {} {}", mode, endpoint);
                    break;
                }
                Err(err) => {
                    if i >= 5 {
                        panic!(
                            "Failed to setup PUB socket at {}: {}. Giving up.",
                            endpoint, err
                        );
                    }
                    log::warn!("PUB: Setup attempt {} failed: {}", i + 1, err);
                    i += 1;
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        sleep(Duration::from_millis(1000)).await;

        Self {
            socket: Arc::new(Mutex::new(publisher)),
        }
    }

    pub async fn new(endpoint: &str) -> Self {
        Self::bind(endpoint).await
    }

    pub async fn send_binary(&self, topic: &str, payload: &[u8]) -> zmq::Result<()> {
        let socket = self.socket.lock().await;

        socket.send(topic, zmq::SNDMORE)?;
        socket.send(payload, 0)?;

        log::debug!("PUB: Message sent: {} | {} bytes", topic, payload.len());
        Ok(())
    }

    pub async fn send(&self, topic: &str, message: impl ToString) -> zmq::Result<()> {
        let full_message = format!("{} {}", topic, message.to_string());
        let socket = self.socket.lock().await;

        match socket.send(full_message.as_bytes(), 0) {
            Ok(_) => {
                log::debug!("PUB: Message sent: {}", full_message);
                Ok(())
            }
            Err(e) => {
                log::error!("PUB: Failed to send message: {}", e);
                Err(e)
            }
        }
    }
}
