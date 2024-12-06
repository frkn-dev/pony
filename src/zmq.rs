use futures::future::join_all;
use log::debug;
use log::error;
use log::info;
use serde::Deserialize;
use std::error::Error;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;
use tokio::time::Duration;
use zmq;

use crate::appconfig::Settings;
use crate::users::UserInfo;
use crate::xray_op::{client, users, vmess, Tag};

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

#[derive(Deserialize, Clone, Debug)]
pub struct Message {
    pub user_id: String,
    pub action: Action,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
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
    state: Arc<Mutex<users::UserState>>,
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

                    let futures = vec![
                        process_message(
                            clients.clone(),
                            message.clone(),
                            Tag::Vmess,
                            state.clone(),
                            config.xray.xray_daily_limit_mb,
                        ),
                        process_message(
                            clients.clone(),
                            message.clone(),
                            Tag::Vless,
                            state.clone(),
                            config.xray.xray_daily_limit_mb,
                        ),
                        process_message(
                            clients.clone(),
                            message.clone(),
                            Tag::Shadowsocks,
                            state.clone(),
                            config.xray.xray_daily_limit_mb,
                        ),
                    ];

                    let _ = join_all(futures).await;
                }
                Err(e) => {
                    eprintln!("Error parsing JSON: {} {:?}", e, message.clone());
                }
            };
        }
    }
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    tag: Tag,
    state: Arc<Mutex<users::UserState>>,
    config_daily_limit_mb: i64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let user_info = UserInfo::new(message.user_id.to_string(), tag.clone());

    match tag {
        Tag::Vmess => match message.action {
            Action::Create => match vmess::add_user(clients.clone(), user_info.clone()).await {
                Ok(()) => {
                    let mut user_state = state.lock().await;
                    let daily_limit_mb = message.limit.unwrap_or(config_daily_limit_mb);
                    let trial = message.trial.unwrap_or(true);
                    let user = users::User::new(user_info.uuid.clone(), daily_limit_mb, trial);
                    let _ = user_state.add_or_update_user(user).await;
                    info!("User add completed successfully {:?}", user_info.uuid);
                    Ok(())
                }
                Err(e) => {
                    error!("User add operations failed: {:?}", e);
                    Err(Box::new(e))
                }
            },
            Action::Delete => {
                match vmess::remove_user(clients.clone(), user_info.uuid.clone(), tag).await {
                    Ok(()) => {
                        let mut user_state = state.lock().await;
                        let _ = user_state.expire_user(&user_info.uuid).await;

                        info!("User remove successfully {:?}", user_info.uuid);
                        Ok(())
                    }
                    Err(e) => {
                        error!("User remove failed: {:?}", e);
                        Err(Box::new(e))
                    }
                }
            }
            Action::Restore => {
                debug!("Restore user {}", user_info.uuid);
                let mut user_state = state.lock().await;
                let _ = user_state.restore_user(user_info.uuid).await;
                Ok(())
            }
            Action::Update => {
                if let Some(trial) = message.trial {
                    debug!("Update user trial {} {}", user_info.uuid, trial);
                    let mut user_state = state.lock().await;
                    let _ = user_state.update_user_trial(&user_info.uuid, trial).await;
                }
                if let Some(limit) = message.limit {
                    debug!("Update user limit {} {}", user_info.uuid, limit);
                    let mut user_state = state.lock().await;
                    let _ = user_state.update_user_limit(&user_info.uuid, limit).await;
                }
                Ok(())
            }
        },
        Tag::Vless => {
            debug!("Vless: Not implemented process_message");
            Ok(())
        }
        Tag::Shadowsocks => {
            debug!("Shadowsocks: Not implemented process_message");
            Ok(())
        }
    }
}
