use log::debug;
use log::error;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;
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

#[derive(Deserialize, Clone, Debug)]
pub struct Message {
    pub user_id: String,
    pub action: Action,
    pub tag: Tag,
    pub trial: Option<bool>,
    pub limit: Option<u64>,
}

pub async fn process_message(
    clients: client::XrayClients,
    message: Message,
    state: Arc<Mutex<users::UserState>>,
) {
    let tag_str = match message.tag {
        Tag::Vless => "Vless",
        Tag::Vmess => "Vmess",
        Tag::Shadowsocks => "Shadowsocks",
    };

    let user_info = users::UserInfo {
        in_tag: tag_str.to_string(),
        level: 0,
        email: format!("{}@{}", message.user_id, tag_str),
        uuid: message.user_id.to_string(),
    };

    match message.tag {
        Tag::Vmess => match message.action {
            Action::Create => match vmess::add_user(clients.clone(), user_info.clone()).await {
                Ok(()) => {
                    let mut user_state = state.lock().await;
                    let user = users::User {
                        user_id: user_info.uuid.clone(),
                        tag: user_info.in_tag,
                        limit: message.limit,
                        trial: message.trial,
                    };
                    user_state.add_user(user);
                    info!("User add completed successfully {:?}", user_info.uuid);
                }
                Err(e) => {
                    error!("User add operations failed: {:?}", e);
                }
            },
            Action::Delete => match vmess::remove_user(clients.clone(), user_info.clone()).await {
                Ok(()) => {
                    let mut user_state = state.lock().await;
                    user_state.remove_user(&user_info.uuid);

                    info!("User remove successfully {:?}", user_info.uuid);
                }
                Err(e) => {
                    error!("User remove failed: {:?}", e);
                }
            },
            Action::Update => {
                if let Some(trial) = message.trial {
                    debug!("Updated user trial {} {}", user_info.uuid, trial);
                    let mut user_state = state.lock().await;
                    user_state.update_user_trial(&user_info.uuid, trial);
                } else {
                    debug!("Provide trial value for user {}", user_info.uuid);
                }
                if let Some(limit) = message.limit {
                    debug!("Updated user limit {} {}", user_info.uuid, limit);
                    let mut user_state = state.lock().await;
                    user_state.update_user_limit(&user_info.uuid, Some(limit));
                } else {
                    debug!("Provide limit for user {}", user_info.uuid);
                }
            }
        },
        Tag::Vless => {
            debug!("Not implemented");
        }
        Tag::Shadowsocks => {
            debug!("Not implemented");
        }
    }
}

pub async fn subscriber(
    clients: client::XrayClients,
    config: ZmqConfig,
    state: Arc<Mutex<users::UserState>>,
) {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();

    subscriber.connect(&config.endpoint).unwrap();
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
                    process_message(clients.clone(), message.clone(), state.clone()).await;
                }
                Err(e) => {
                    eprintln!("Error parsing JSON: {} {:?}", e, message.clone());
                }
            };
        }
    }
}
