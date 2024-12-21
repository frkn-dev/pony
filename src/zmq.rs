use log::{debug, error, info};
use std::{sync::Arc, thread};
use tokio::{sync::Mutex, time::Duration};
use zmq;

use crate::utils::measure_time;

use super::{
    message::{process_message, Message},
    settings::Settings,
};

use super::{state::State, xray_op::client};

fn try_connect(endpoint: &str, topic: &str) -> zmq::Socket {
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

    assert!(subscriber.set_subscribe(topic.as_bytes()).is_ok());
    subscriber
}

pub async fn subscriber(
    clients: client::XrayClients,
    settings: Settings,
    state: Arc<Mutex<State>>,
) {
    let subscriber = try_connect(&settings.zmq.endpoint, &settings.node.env);

    info!(
        "Subscriber connected to {}:{}",
        settings.zmq.endpoint, settings.node.env
    );

    loop {
        match subscriber.recv_string(0) {
            Ok(Ok(data)) => {
                let data = data.to_string();
                tokio::spawn({
                    let clients = clients.clone();
                    let state = state.clone();
                    let settings = settings.clone();

                    async move {
                        let mut parts = data.splitn(2, ' ');
                        let topic = parts.next().unwrap_or("");
                        let payload = parts.next().unwrap_or("");

                        if topic != settings.node.env {
                            return;
                        }

                        match serde_json::from_str::<Message>(payload) {
                            Ok(message) => {
                                info!("SUB: Message received: {:?}", message);

                                if let Err(err) = measure_time(
                                    process_message(
                                        clients,
                                        message.clone(),
                                        state,
                                        settings.xray.xray_daily_limit_mb,
                                    ),
                                    "process_message".to_string(),
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
                });
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
