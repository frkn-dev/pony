use log::debug;
use log::error;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::time::Duration;

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub in_tag: String,
    pub level: u32,
    pub email: String,
    pub uuid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub user_id: String,
    pub tag: String,
    pub trial: Option<bool>,
    pub limit: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserState {
    pub users: Vec<User>,
}

impl UserState {
    pub fn new() -> Self {
        UserState { users: Vec::new() }
    }

    pub fn add_user(&mut self, user: User) {
        self.users.push(user);
    }

    pub fn remove_user(&mut self, user_id: &str) {
        self.users.retain(|user| user.user_id != user_id);
    }

    pub fn update_user_limit(&mut self, user_id: &str, new_limit: Option<u64>) {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.limit = new_limit;
        }
    }

    pub fn update_user_trial(&mut self, user_id: &str, new_trial: bool) {
        if let Some(user) = self.users.iter_mut().find(|user| user.user_id == user_id) {
            user.trial = Some(new_trial);
        }
    }
    pub async fn load_from_file_async(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let mut file = fs::File::open(file_path).await?;
        let mut file_content = String::new();
        file.read_to_string(&mut file_content).await?;

        let user_state: UserState = serde_json::from_str(&file_content)?;
        debug!("State {:?}", user_state);

        Ok(user_state)
    }

    pub async fn save_to_file_async(&self, file_path: &str) -> Result<(), Box<dyn Error>> {
        let file_content = serde_json::to_string_pretty(&self)?;

        match tokio::fs::write(file_path, file_content).await {
            Ok(_) => {
                debug!("Written successfully");
            }
            Err(e) => {
                error!("Write file failed: {:?}", e);
            }
        }

        Ok(())
    }
}

pub async fn periodic_flush(file_path: String, user_state: Arc<Mutex<UserState>>) {
    debug!("Run flushing state to file");
    loop {
        sleep(Duration::from_secs(2)).await;

        debug!("Flushing to file");
        let user_state = user_state.lock().await;

        if let Err(e) = user_state.save_to_file_async(&file_path).await {
            error!("Failed to flush state to file: {:?}", e);
        }
    }
}
