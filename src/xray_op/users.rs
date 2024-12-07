use crate::xray_op::vmess;
use chrono::{DateTime, Utc};
use log::debug;
use log::error;
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::client::XrayClients;
use super::user_state::UserState;
use super::Tag;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    Active,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub user_id: String,
    pub trial: bool,
    pub limit: i64,
    pub status: UserStatus,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub modified_at: Option<DateTime<Utc>>,
    pub proto: Option<Vec<Tag>>,
}

impl User {
    pub fn new(user_id: String, limit: i64, trial: bool) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            trial: trial,
            limit: limit,
            status: UserStatus::Active,
            uplink: None,
            downlink: None,
            created_at: now,
            modified_at: None,
            proto: None,
        }
    }

    pub fn update_modified_at(&mut self) {
        self.modified_at = Some(Utc::now());
    }

    pub fn add_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            if !proto.contains(&tag) {
                proto.push(tag);
            }
        } else {
            self.proto = Some(vec![tag]);
        }
    }

    pub fn remove_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            proto.retain(|p| *p != tag);
            if proto.is_empty() {
                self.proto = None;
            }
        }
    }

    pub fn has_proto_tag(&self, tag: Tag) -> bool {
        if let Some(proto_tags) = &self.proto {
            return proto_tags.contains(&tag);
        }
        false
    }
}

pub async fn check_and_block_user(
    clients: XrayClients,
    state: Arc<Mutex<UserState>>,
    user_id: &str,
    tag: Tag,
) {
    let mut user_state = state.lock().await;

    if let Some(user) = user_state
        .users
        .iter_mut()
        .find(|user| user.user_id == user_id)
    {
        if let (limit, Some(downlink), trial, status) =
            (user.limit, user.downlink, user.trial, user.status.clone())
        {
            if trial && status == UserStatus::Active && downlink > limit {
                let user_id_copy = user.user_id.clone();

                match tag {
                    Tag::Vmess => {
                        drop(user_state);

                        match vmess::remove_user(clients.clone(), format!("{user_id_copy}@{tag}"))
                            .await
                        {
                            Ok(()) => {
                                let mut user_state = state.lock().await;
                                let _ = user_state.expire_user(&user_id_copy).await;
                                info!("User removed successfully: {:?}", user_id_copy);
                            }
                            Err(e) => {
                                error!("Failed to remove user: {:?}", e);
                            }
                        }
                    }
                    Tag::Vless => debug!("Vless: not implemented"),
                    Tag::Shadowsocks => debug!("Shadowsocks: not implemented"),
                }
            }
        }
    }
}
