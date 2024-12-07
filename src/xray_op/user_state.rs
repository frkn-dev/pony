use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::fs;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use super::client::XrayClients;
use super::stats::StatType;
use super::users::{User, UserStatus};
use super::Tag;
use super::{vless, vmess};

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

    pub async fn add_user(&mut self, new_user: User) -> Result<(), Box<dyn Error>> {
        debug!("Starting add_or_update_user for user: {:?}", new_user);

        if let Some(existing_user) = self
            .users
            .iter_mut()
            .find(|user| user.user_id == new_user.user_id)
        {
            debug!("User {} already exist", existing_user.user_id);
        } else {
            debug!("Adding new user: {:?}", new_user);
            self.users.push(new_user);
        }

        self.save_to_file_async().await?;
        Ok(())
    }

    pub async fn restore_user(&mut self, user: User) -> Result<(), Box<dyn Error>> {
        if let Some(existing_user) = self.users.iter_mut().find(|u| u.user_id == user.user_id) {
            debug!("Restoring {}", existing_user.user_id);
            existing_user.status = UserStatus::Active;
            existing_user.update_modified_at();
            existing_user.limit = user.limit;
            existing_user.trial = user.trial;
        } else {
            error!("User not found {} ", user.user_id);
        }
        self.save_to_file_async().await?;
        Ok(())
    }

    pub async fn expire_user(&mut self, user_id: &str) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.status = UserStatus::Expired;
            user.update_modified_at();
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
            user.limit = new_limit;
            user.update_modified_at();
        } else {
            error!("User not found: {:?} ", user_id);
        }
        self.save_to_file_async().await?;

        Ok(())
    }

    pub async fn update_user_trial(
        &mut self,
        user_id: &str,
        new_trial: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.trial = new_trial;
            user.update_modified_at();
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

    pub fn get_all_trial_users(&self, status: UserStatus) -> Vec<User> {
        let users = self
            .users
            .iter()
            .filter(|user| user.status == status && user.trial)
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

pub async fn sync_state_to_xray_conf(
    state: Arc<Mutex<UserState>>,
    clients: XrayClients,
    tag: Tag,
) -> Result<(), Box<dyn Error>> {
    let state = state.lock().await;
    let users = state.users.clone();

    for user in &users {
        debug!("Running sync for {:?} {:?}", tag.clone(), user);
        match tag {
            Tag::Vmess => {
                let user_info = vmess::UserInfo::new(user.user_id.clone(), tag.clone());
                if user.has_proto_tag(tag.clone()) {
                    match user.status {
                        UserStatus::Active => {
                            match vmess::add_user(clients.clone(), user_info.clone()).await {
                                Ok(()) => debug!("User sync success {:?} {}", user_info, tag),
                                Err(e) => error!("User sync fail {:?} {}", user_info, e),
                            }
                        }
                        UserStatus::Expired => debug!("User expired, skip to restore"),
                    }
                }
            }

            Tag::Vless => {
                let user_info = vless::UserInfo::new(user.user_id.clone(), tag.clone());
                if user.has_proto_tag(tag.clone()) {
                    match user.status {
                        UserStatus::Active => {
                            match vless::add_user(clients.clone(), user_info.clone()).await {
                                Ok(()) => debug!("User sync success {:?} {}", user_info, tag),
                                Err(e) => error!("User sync fail {:?} {}", user_info, e),
                            }
                        }
                        UserStatus::Expired => debug!("User expired, skip to restore"),
                    }
                }
            }
            Tag::Shadowsocks => debug!("ShadowSocks: Not implemented"),
        }
    }

    Ok(())
}
