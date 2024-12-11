use std::{sync::Arc, thread};

use futures::future::join_all;
use log::{debug, error, info};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::Duration;
use zmq;

use super::appconfig::Settings;
use super::message::{process_message, Message};

use crate::user_state::UserState;
use crate::xray_op::client;

#[derive(Deserialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update,
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
    state: Arc<Mutex<UserState>>,
    debug: bool,
) {
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

                    let state = state.clone();

                    let futures = vec![process_message(
                        clients.clone(),
                        message.clone(),
                        state.clone(),
                        config.xray.xray_daily_limit_mb,
                        debug,
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
