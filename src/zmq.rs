use std::{sync::Arc, thread};

use futures::future::join_all;
use log::{debug, error, info};
use tokio::sync::Mutex;
use tokio::time::Duration;
use zmq;

use super::appconfig::Settings;
use super::message::{process_message, Message};

use crate::user_state::UserState;
use crate::xray_op::client;

fn try_connect(endpoint: &str) -> zmq::Socket {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).expect("Failed to create socket");

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
    settings: Settings,
    state: Arc<Mutex<UserState>>,
    debug: bool,
) {
    let subscriber = try_connect(&settings.zmq.endpoint);
    assert!(subscriber
        .set_subscribe(settings.zmq.topic.as_bytes())
        .is_ok());

    info!(
        "Subscriber connected to {}:{}",
        settings.zmq.endpoint, settings.zmq.topic
    );

    loop {
        match subscriber.recv_string(0) {
            Ok(Ok(data)) => {
                let mut parts = data.splitn(2, ' ');
                let topic = parts.next().unwrap_or("");
                let payload = parts.next().unwrap_or("");

                if topic != settings.zmq.topic {
                    continue;
                }

                match serde_json::from_str::<Message>(payload) {
                    Ok(message) => {
                        debug!("SUB: Message received: {:?}", message);

                        let state = state.clone();
                        if let Err(err) = process_message(
                            clients.clone(),
                            message,
                            state,
                            settings.xray.xray_daily_limit_mb,
                            debug,
                        )
                        .await
                        {
                            error!("SUB: Error processing message: {:?}", err);
                        }
                    }
                    Err(e) => {
                        error!("SUB: Error parsing JSON: {}. Data: {}", e, payload);
                    }
                }
            }
            Ok(Err(e)) => {
                error!("SUB: Failed to decode message: {:?}", e);
            }
            Err(e) => {
                error!("SUB: Failed to receive message: {:?}", e);
            }
        }
    }
}
