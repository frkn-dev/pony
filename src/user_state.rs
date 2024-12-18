use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    error::Error,
};
use tokio::{
    fs,
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::xray_op::{stats::StatType, Tag};

use super::{
    node::{Inbound, Node},
    settings::Settings,
    user::{User, UserStatus},
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserState {
    pub file_path: String,
    pub users: HashMap<String, User>,
    pub node: Node,
}

impl UserState {
    pub fn new(settings: Settings, inbounds: HashMap<Tag, Inbound>) -> Self {
        UserState {
            users: HashMap::new(),
            file_path: settings.app.file_state,
            node: Node::new(
                inbounds,
                settings.node.hostname.expect("hostname"),
                settings.node.ipv4.expect("ipv4addr"),
            ),
        }
    }

    pub async fn add_user(
        &mut self,
        user_id: String,
        new_user: User,
    ) -> Result<(), Box<dyn Error>> {
        debug!("Starting add_user for user: {:?}", new_user);

        match self.users.entry(user_id.clone()) {
            Entry::Occupied(mut entry) => {
                debug!("User {} already exists: {:?}", user_id, entry.get());
                let existing_user = entry.get_mut();
                existing_user.trial = new_user.trial;
                existing_user.limit = new_user.limit;
                existing_user.password = new_user.password;
                existing_user.status = new_user.status;
            }
            Entry::Vacant(entry) => {
                debug!("Adding new user: {:?}", new_user);
                entry.insert(new_user);
            }
        }

        Ok(())
    }

    pub async fn restore_user(&mut self, user_id: &str) -> Result<(), Box<dyn Error>> {
        if let Some(existing_user) = self.users.get_mut(user_id) {
            debug!("Restoring {}", user_id);
            existing_user.status = UserStatus::Active;
            existing_user.update_modified_at();
            Ok(())
        } else {
            error!("User not found: {}", user_id);
            Err("User not found".into())
        }
    }

    pub async fn expire_user(&mut self, user_id: &str) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(user_id) {
            user.status = UserStatus::Expired;
            user.update_modified_at();
            debug!("User {} marked as expired", user_id);
        } else {
            error!("User not found: {:?}", user_id);
        }

        Ok(())
    }

    pub async fn update_user_limit(
        &mut self,
        user_id: &str,
        new_limit: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(user_id) {
            user.limit = new_limit;
            user.update_modified_at();
            debug!("User {} limit updated to {}", user_id, new_limit);
        } else {
            error!("User not found: {:?}", user_id);
        }

        Ok(())
    }

    pub async fn update_user_trial(
        &mut self,
        user_id: &str,
        new_trial: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(user_id) {
            user.trial = new_trial;
            user.update_modified_at();
            debug!("User {} trial status updated to {}", user_id, new_trial);
        } else {
            error!("User not found: {}", user_id);
        }

        Ok(())
    }

    pub async fn update_user_stat(
        &mut self,
        user_id: &str,
        stat: StatType,
        new_value: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(user_id) {
            match stat {
                StatType::Uplink => {
                    if let Some(value) = new_value {
                        user.uplink = Some(value);
                    } else {
                        return Err("New value for Uplink is None".into());
                    }
                }
                StatType::Downlink => {
                    if let Some(value) = new_value {
                        user.downlink = Some(value);
                    } else {
                        return Err("New value for Downlink is None".into());
                    }
                }
            }
            user.update_modified_at();
            debug!("Updated {:?} for user {}", stat, user_id);
            Ok(())
        } else {
            let err_msg = format!("User not found: {}", user_id);
            error!("{}", err_msg);
            Err(err_msg.into())
        }
    }

    pub async fn update_user_uplink(
        &mut self,
        user_id: &str,
        new_uplink: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.update_user_stat(user_id, StatType::Uplink, Some(new_uplink))
            .await
    }

    pub async fn update_user_downlink(
        &mut self,
        user_id: &str,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.update_user_stat(user_id, StatType::Downlink, Some(new_downlink))
            .await
    }

    pub fn reset_user_stat(&mut self, user_id: &str, stat: StatType) {
        if let Some(user) = self.users.get_mut(user_id) {
            match stat {
                StatType::Uplink => user.reset_uplink(),
                StatType::Downlink => user.reset_downlink(),
            }
            debug!("Reset {:?} for user {}", stat, user_id);
        } else {
            error!("User not found: {}", user_id);
        }
    }

    pub async fn update_node_uplink(
        &mut self,
        tag: Tag,
        new_uplink: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.node.update_uplink(tag, new_uplink)?;
        Ok(())
    }

    pub async fn update_node_downlink(
        &mut self,
        tag: Tag,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.node.update_downlink(tag, new_downlink)?;
        Ok(())
    }

    pub async fn update_node_user_count(
        &mut self,
        tag: Tag,
        user_count: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.node.update_user_count(tag, user_count)?;
        Ok(())
    }

    pub fn get_all_trial_users(&self, status: UserStatus) -> HashMap<String, User> {
        self.users
            .iter()
            .filter(|(_, user)| user.status == status && user.trial)
            .map(|(user_id, user)| (user_id.clone(), user.clone()))
            .collect()
    }

    pub async fn load_from_file_async(file_path: String) -> Result<Self, Box<dyn Error>> {
        let mut file = fs::File::open(file_path).await?;
        let mut file_content = String::new();
        file.read_to_string(&mut file_content).await?;

        let user_state: UserState = serde_json::from_str(&file_content)?;
        debug!("State loaded {:?}", user_state);

        Ok(user_state)
    }

    pub async fn save_to_file_async(&self, msg: &str) -> Result<(), Box<dyn Error>> {
        let file_content = serde_json::to_string_pretty(&self)?;

        let mut file = File::create(self.file_path.clone()).await?;
        file.write_all(file_content.as_bytes()).await?;
        file.sync_all().await?;

        debug!("{msg}: Written successfully");
        Ok(())
    }
}
