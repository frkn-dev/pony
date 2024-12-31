use crate::message::Message;
use log::debug;
use log::error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use zmq;

pub async fn publisher(endpoint: &str) -> Arc<Mutex<zmq::Socket>> {
    let context = zmq::Context::new();
    let publisher = context.socket(zmq::PUB).expect("Failed to create socket");

    let mut i = 0;
    loop {
        match publisher.bind(endpoint) {
            Ok(_) => {
                debug!("Connected to Pub socket at {}", endpoint);
                break;
            }
            Err(err) => {
                if i == 5 {
                    panic!(
                        "Failed to connect to Pub socket at {}: {}. Retrying...",
                        endpoint, err
                    );
                }
                debug!("Trying to connect Pub socket, attempt {} {}", i, err);
                i = i + 1;
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    // This delay is an uglyhack for zmq pub/sub
    sleep(Duration::from_millis(1000)).await;

    Arc::new(Mutex::new(publisher))
}

pub async fn send_message(
    publisher: Arc<Mutex<zmq::Socket>>,
    topic: &str,
    message: Message,
) -> zmq::Result<()> {
    let full_message = format!("{} {}", topic, message);

    let publisher = publisher.lock().await;

    match publisher.send(full_message.as_bytes(), 0) {
        Ok(_) => {
            if let Ok(_) = publisher.send(full_message.as_bytes(), 0) {
                debug!("PUB: Message sent: {}", full_message);
            }
            return Ok(());
        }
        Err(e) => {
            error!("PUB: Failed to send message: {}", e);
            return Err(e);
        }
    }
}
