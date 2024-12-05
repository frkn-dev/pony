use log::debug;
use log::error;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

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
    pub limit: Option<u64>,
    pub status: UserStatus,
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
        new_limit: u64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.limit = Some(new_limit);
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
