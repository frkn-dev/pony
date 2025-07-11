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
    pub async fn new(endpoint: &str) -> Self {
        let context = zmq::Context::new();
        let publisher = context.socket(zmq::PUB).expect("Failed to create socket");

        let mut i = 0;
        loop {
            match publisher.bind(endpoint) {
                Ok(_) => {
                    log::debug!("Connected to Pub socket at {}", endpoint);
                    break;
                }
                Err(err) => {
                    if i >= 5 {
                        panic!(
                            "Failed to connect to Pub socket at {}: {}. Giving up.",
                            endpoint, err
                        );
                    }
                    log::debug!("Trying to connect Pub socket, attempt {}: {}", i + 1, err);
                    i += 1;
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        // Ugly hack for zmq pub/sub sync
        sleep(Duration::from_millis(1000)).await;

        Self {
            socket: Arc::new(Mutex::new(publisher)),
        }
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
