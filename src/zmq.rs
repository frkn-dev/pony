use futures::future::join_all;
use log::debug;
use log::error;
use log::info;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;
use tokio::time::Duration;
use zmq;

use crate::appconfig::ZmqConfig;
use crate::xray_op::{client, users, vmess};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub enum Action {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update,
}

#[derive(Deserialize, Debug, Clone)]
pub enum Tag {
    #[serde(rename = "vless")]
    Vless,
    #[serde(rename = "vmess")]
    Vmess,
    #[serde(rename = "shadowsocks")]
    Shadowsocks,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Vless => write!(f, "Vless"),
            Tag::Vmess => write!(f, "Vmess"),
            Tag::Shadowsocks => write!(f, "Shadowsocks"),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Message {
    pub user_id: String,
    pub action: Action,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    tag: Tag,
    state: Arc<Mutex<users::UserState>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let user_info = users::UserInfo {
        in_tag: tag.to_string(),
        level: 0,
        email: format!("{}@{}", message.user_id, tag),
        uuid: message.user_id.to_string(),
    };

    match tag {
        Tag::Vmess => match message.action {
            Action::Create => match vmess::add_user(clients.clone(), user_info.clone()).await {
                Ok(()) => {
                    let mut user_state = state.lock().await;
                    let user = users::User {
                        user_id: user_info.uuid.clone(),
                        limit: message.limit,
                        trial: message.trial,
                        status: users::UserStatus::Active,
                        uplink: None,
                        downlink: None,
                    };
                    let _ = user_state.add_or_update_user(user).await;
                    info!("User add completed successfully {:?}", user_info.uuid);
                    Ok(())
                }
                Err(e) => {
                    error!("User add operations failed: {:?}", e);
                    Err(Box::new(e))
                }
            },
            Action::Delete => match vmess::remove_user(clients.clone(), user_info.email, tag).await
            {
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
            },
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
    config: ZmqConfig,
    state: Arc<Mutex<users::UserState>>,
) {
    let _context = zmq::Context::new();

    let subscriber = try_connect(&config.endpoint);
    subscriber.set_subscribe(config.topic.as_bytes()).unwrap();

    info!(
        "Subscriber connected to {}:{}",
        config.endpoint, config.topic
    );

    loop {
        let message = subscriber.recv_string(0).unwrap();
        if let Ok(ref data) = message {
            if data.trim() == config.topic {
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
                        ),
                        process_message(
                            clients.clone(),
                            message.clone(),
                            Tag::Vless,
                            state.clone(),
                        ),
                        process_message(
                            clients.clone(),
                            message.clone(),
                            Tag::Shadowsocks,
                            state.clone(),
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
