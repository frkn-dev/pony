use futures::future::join_all;
use log::debug;
use log::error;
use log::info;
use serde::Deserialize;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;
use tokio::time::Duration;
use zmq;

use super::message::{process_message, Message};
use crate::appconfig::Settings;
use crate::xray_op::{client, user_state};

#[derive(Deserialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "restore")]
    Restore,
}

fn try_connect(endpoint: &str) -> zmq::Socket {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).expect("Failed to create socket");

    subscriber
        .set_subscribe(b"")
        .expect("Failed to set subscription");

    loop {
        match subscriber.connect(endpoint) {
            Ok(_) => {
                debug!("Connected to publisher at {}", endpoint);
                break;
            }
            Err(err) => {
                error!(
                    "Failed to connect to publisher at {}: {}. Retrying...",
                    endpoint, err
                );
                thread::sleep(Duration::from_secs(5));
            }
        }
    }

    subscriber
}

pub async fn subscriber(
    clients: client::XrayClients,
    config: Settings,
    state: Arc<Mutex<user_state::UserState>>,
) {
    let _context = zmq::Context::new();

    let subscriber = try_connect(&config.zmq.endpoint);
    subscriber
        .set_subscribe(config.zmq.topic.as_bytes())
        .unwrap();

    info!(
        "Subscriber connected to {}:{}",
        config.zmq.endpoint, config.zmq.topic
    );

    loop {
        let message = subscriber.recv_string(0).unwrap();
        if let Ok(ref data) = message {
            if data.trim() == config.zmq.topic {
                continue;
            }
            match serde_json::from_str::<Message>(&data) {
                Ok(message) => {
                    debug!("Message recieved {:?}", message.clone());

                    let futures = vec![process_message(
                        clients.clone(),
                        message.clone(),
                        state.clone(),
                        config.xray.xray_daily_limit_mb,
                    )];

                    let _ = join_all(futures).await;
                }
                Err(e) => {
                    eprintln!("Error parsing JSON: {} {:?}", e, message.clone());
                }
            };
        }
    }
}
