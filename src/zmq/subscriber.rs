use crate::state::user::User;
use crate::xray_op::client::HandlerClient;
use log::{debug, error, info};
use std::error::Error;
use std::{sync::Arc, thread};
use tokio::{sync::Mutex, time::Duration};
use zmq;

use super::message::{Action, Message};
use crate::xray_op::actions::{create_users, remove_users};
use crate::{config::settings::AgentSettings, state::state::State, utils::measure_time};

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
    client: Arc<Mutex<HandlerClient>>,
    settings: AgentSettings,
    state: Arc<Mutex<State>>,
) {
    let subscriber = try_connect(&settings.zmq.sub_endpoint, &settings.node.env);

    info!(
        "Subscriber connected to {}:{}",
        settings.zmq.sub_endpoint, settings.node.env
    );

    loop {
        tokio::task::yield_now().await;
        match subscriber.recv_string(0) {
            Ok(Ok(data)) => {
                let data = data.to_string();
                tokio::spawn({
                    let clients = client.clone();
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

pub async fn process_message(
    client: Arc<Mutex<HandlerClient>>,
    message: Message,
    state: Arc<Mutex<State>>,
    config_daily_limit_mb: i64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match message.action {
        Action::Create => {
            let user_id = message.user_id;

            let daily_limit_mb = message.limit.unwrap_or(config_daily_limit_mb);
            let trial = message.trial.unwrap_or(true);

            let user = User::new(trial, daily_limit_mb, message.env, message.password.clone());

            println!("USER {:?}", user);

            match create_users(message.user_id.clone(), message.password.clone(), client).await {
                Ok(_) => {
                    let mut state_guard = state.lock().await;

                    state_guard
                        .add_user(user_id.clone(), user.clone())
                        .await
                        .map_err(|err| {
                            error!("Failed to add user {}: {:?}", message.user_id, err);
                            format!("Failed to add user {}", message.user_id).into()
                        })
                }
                Err(err) => {
                    error!("Failed to create user {}: {:?}", message.user_id, err);
                    Err(format!("Failed to create user {}", message.user_id).into())
                }
            }
        }
        Action::Delete => {
            if let Err(e) = remove_users(message.user_id, client).await {
                return Err(format!("Couldn't remove users from Xray: {}", e).into());
            } else {
                let mut state = state.lock().await;
                let _ = state.expire_user(message.user_id).await;
            }

            Ok(())
        }
        Action::Update => {
            let mut state_lock = state.lock().await;

            if let Some(user) = state_lock.get_user(message.user_id).await {
                if let Some(trial) = message.trial {
                    if trial != user.trial {
                        if let Err(e) =
                            create_users(message.user_id, message.password.clone(), client).await
                        {
                            return Err(format!(
                                "Couldn’t update trial for user {}: {}",
                                message.user_id, e
                            )
                            .into());
                        }

                        if let Err(e) = state_lock.update_user_trial(message.user_id, trial).await {
                            return Err(format!(
                                "Failed to update trial for user {}: {}",
                                message.user_id, e
                            )
                            .into());
                        }
                    }
                }

                if let Some(limit) = message.limit {
                    if let Err(e) = state_lock.update_user_limit(message.user_id, limit).await {
                        return Err(format!(
                            "Failed to update limit for user {}: {}",
                            message.user_id, e
                        )
                        .into());
                    }
                }

                Ok(())
            } else {
                Err(format!("User {} not found", message.user_id).into())
            }
        }
    }
}
