use crate::xray_op::vmess;
use log::debug;
use log::error;
use log::info;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::client::XrayClients;
use super::stats::StatType;
use crate::zmq::Tag;

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub in_tag: String,
    pub level: u32,
    pub email: String,
    pub uuid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    Active,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub user_id: String,
    pub trial: Option<bool>,
    pub limit: Option<i64>,
    pub status: UserStatus,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserState {
    pub file_path: String,
    pub users: Vec<User>,
}

impl UserState {
    pub fn new(file_path: String) -> Self {
        UserState {
            users: Vec::new(),
            file_path: file_path,
        }
    }

    pub async fn add_or_update_user(&mut self, new_user: User) -> Result<(), Box<dyn Error>> {
        debug!("Starting add_or_update_user for user: {:?}", new_user);

        if let Some(existing_user) = self
            .users
            .iter_mut()
            .find(|user| user.user_id == new_user.user_id)
        {
            debug!(
                "Updating existing user: {:?} {:?}",
                existing_user,
                new_user.clone()
            );
            existing_user.status = new_user.status;
            if let Some(trial) = new_user.trial {
                existing_user.trial = Some(trial);
            }
            if let Some(limit) = new_user.limit {
                existing_user.limit = Some(limit);
            }
        } else {
            debug!("Adding new user: {:?}", new_user);
            let user = User {
                user_id: new_user.user_id.clone(),
                trial: new_user.trial.or(Some(true)),
                limit: new_user.limit,
                status: new_user.status,
                uplink: None,
                downlink: None,
            };
            self.users.push(user);
        }

        self.save_to_file_async().await?;

        Ok(())
    }

    pub async fn expire_user(&mut self, user_id: &str) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.status = UserStatus::Expired;
        } else {
            error!("User not found: {:?} ", user_id);
        }
        self.save_to_file_async().await?;

        Ok(())
    }

    pub async fn update_user_limit(
        &mut self,
        user_id: &str,
        new_limit: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.limit = Some(new_limit);
        } else {
            error!("User not found: {:?} ", user_id);
        }
        self.save_to_file_async().await?;

        Ok(())
    }

    pub fn update_user_stat(&mut self, user_id: &str, stat: StatType, new_value: Option<i64>) {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            match stat {
                StatType::Uplink => user.uplink = new_value,
                StatType::Downlink => user.downlink = new_value,
            }
        } else {
            error!("User not found: {}", user_id);
        }
    }

    pub fn update_user_uplink(&mut self, user_id: &str, new_uplink: i64) {
        self.update_user_stat(user_id, StatType::Uplink, Some(new_uplink));
    }

    pub fn update_user_downlink(&mut self, user_id: &str, new_downlink: i64) {
        self.update_user_stat(user_id, StatType::Downlink, Some(new_downlink));
    }

    pub async fn update_user_trial(
        &mut self,
        user_id: &str,
        new_trial: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.trial = Some(new_trial);
        }
        self.save_to_file_async().await?;

        Ok(())
    }

    pub fn get_all_active_users(&self) -> Vec<User> {
        let users = self
            .users
            .iter()
            .filter(|user| user.status == UserStatus::Active)
            .cloned()
            .collect();
        users
    }

    pub async fn load_from_file_async(file_path: String) -> Result<Self, Box<dyn Error>> {
        let mut file = fs::File::open(file_path).await?;
        let mut file_content = String::new();
        file.read_to_string(&mut file_content).await?;

        let user_state: UserState = serde_json::from_str(&file_content)?;
        debug!("State {:?}", user_state);

        Ok(user_state)
    }

    pub async fn save_to_file_async(&self) -> Result<(), Box<dyn Error>> {
        let file_content = serde_json::to_string_pretty(&self)?;

        let mut file = File::create(self.file_path.clone()).await?;
        file.write_all(file_content.as_bytes()).await?;
        file.sync_all().await?;

        debug!("Written successfully");
        Ok(())
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
        if let (Some(limit), Some(downlink), Some(trial)) = (user.limit, user.downlink, user.trial)
        {
            if trial {
                if downlink > limit {
                    match tag {
                        Tag::Vmess => {
                            match vmess::remove_user(clients, format!("{user_id}@{tag}"), tag).await
                            {
                                Ok(()) => {
                                    let mut user_state = state.lock().await;
                                    let _ = user_state.expire_user(&user.user_id).await;

                                    info!("User remove successfully {:?}", user.user_id);
                                }
                                Err(e) => {
                                    error!("User remove failed: {:?}", e);
                                }
                            }
                        }
                        Tag::Vless => debug!("Vless: not implemented"),
                        Tag::Shadowsocks => debug!("Shadowsocks: not implemented"),
                    }
                }
            }
        } else {
            error!("User not found: {}", user_id);
        }
    }
}

pub async fn sync_state_to_xray_conf(
    state: Arc<Mutex<UserState>>,
    clients: XrayClients,
    tag: Tag,
) -> Result<(), Box<dyn Error>> {
    let state = state.lock().await;
    let users = state.get_all_active_users();

    for user in &users {
        debug!("Running sync for {:?} {:?}", tag, user);
        match tag {
            Tag::Vmess => {
                let user_info = UserInfo {
                    uuid: user.user_id.clone(),
                    email: format!("{}@{}", user.user_id, tag),
                    level: 0,
                    in_tag: tag.to_string(),
                };
                match vmess::add_user(clients.clone(), user_info.clone()).await {
                    Ok(()) => debug!("User sync success {:?}", user_info),
                    Err(e) => error!("User sync fail {:?} {}", user_info, e),
                }
            }

            Tag::Vless => debug!("Vless: Not implemented"),
            Tag::Shadowsocks => debug!("ShadowSocks: Not implemented"),
        }
    }

    Ok(())
}
