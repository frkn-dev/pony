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
use uuid::Uuid;

use crate::xray_op::{stats::StatType, Tag};

use super::{
    node::{Inbound, Node},
    settings::Settings,
    user::{User, UserStatus},
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct State {
    pub file_path: String,
    pub users: HashMap<Uuid, User>,
    pub node: Node,
}

impl State {
    pub fn new(settings: Settings, inbounds: HashMap<Tag, Inbound>) -> Self {
        State {
            users: HashMap::new(),
            file_path: settings.app.file_state,
            node: Node::new(
                inbounds,
                settings.node.hostname.expect("hostname"),
                settings.node.ipv4.expect("ipv4addr"),
                settings.node.env,
                settings.node.uuid,
            ),
        }
    }

    pub async fn add_user(&mut self, user_id: Uuid, new_user: User) -> Result<(), Box<dyn Error>> {
        match self.users.entry(user_id) {
            Entry::Occupied(mut entry) => {
                debug!(
                    "User {} already exists, updating: {:?}",
                    user_id,
                    entry.get()
                );
                let existing_user = entry.get_mut();
                existing_user.trial = new_user.trial;
                existing_user.limit = new_user.limit;
                existing_user.password = new_user.password;
                existing_user.status = new_user.status;
                Ok(())
            }
            Entry::Vacant(entry) => {
                debug!("Adding new user: {:?}", new_user);
                entry.insert(new_user.clone());
                Ok(())
            }
        }
    }

    pub async fn restore_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>> {
        if let Some(existing_user) = self.users.get_mut(&user_id) {
            existing_user.status = UserStatus::Active;
            existing_user.update_modified_at();
            Ok(())
        } else {
            Err("User not found".into())
        }
    }

    pub async fn remove_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>> {
        if self.users.remove(&user_id).is_some() {
            Ok(())
        } else {
            Err("User doesn't exist".into())
        }
    }

    pub async fn expire_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.status = UserStatus::Expired;
            user.update_modified_at();
            Ok(())
        } else {
            Err(format!("User not found: {}", user_id).into())
        }
    }

    pub async fn get_user(&self, user_id: Uuid) -> Option<User> {
        self.users.get(&user_id).cloned()
    }

    pub async fn update_user_limit(
        &mut self,
        user_id: Uuid,
        new_limit: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.limit = new_limit;
            user.update_modified_at();
        } else {
            error!("User not found: {:?}", user_id);
        }

        Ok(())
    }

    pub async fn update_user_trial(
        &mut self,
        user_id: Uuid,
        new_trial: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.trial = new_trial;
            user.update_modified_at();
        } else {
            return Err(format!("User not found: {}", user_id).into());
        }

        Ok(())
    }

    pub async fn update_user_stat(
        &mut self,
        user_id: Uuid,
        stat: StatType,
        new_value: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
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
            Ok(())
        } else {
            let err_msg = format!("User not found: {}", user_id);
            Err(err_msg.into())
        }
    }

    pub async fn update_user_uplink(
        &mut self,
        user_id: Uuid,
        new_uplink: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.update_user_stat(user_id, StatType::Uplink, Some(new_uplink))
            .await
    }

    pub async fn update_user_downlink(
        &mut self,
        user_id: Uuid,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.update_user_stat(user_id, StatType::Downlink, Some(new_downlink))
            .await
    }

    pub fn reset_user_stat(&mut self, user_id: Uuid, stat: StatType) {
        if let Some(user) = self.users.get_mut(&user_id) {
            match stat {
                StatType::Uplink => user.reset_uplink(),
                StatType::Downlink => user.reset_downlink(),
            }
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

    pub fn get_all_trial_users(&self, status: UserStatus) -> HashMap<Uuid, User> {
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

        let user_state: State = serde_json::from_str(&file_content)?;
        debug!("State loaded {:?}", user_state);

        Ok(user_state)
    }

    pub async fn save_to_file_async(&self, msg: &str) -> Result<(), Box<dyn Error>> {
        let file_content = match serde_json::to_string_pretty(&self) {
            Ok(content) => {
                debug!("content len to save {}", content.len());
                content
            }
            Err(e) => {
                error!("{msg}: Failed to serialize state to JSON: {:?}", e);
                return Err(Box::new(e));
            }
        };

        let file_path = self.file_path.clone();
        let mut file = match File::create(&file_path).await {
            Ok(f) => f,
            Err(e) => {
                error!("{msg}: Failed to create file at {:?}: {:?}", file_path, e);
                return Err(Box::new(e));
            }
        };

        if let Err(e) = file.write_all(file_content.as_bytes()).await {
            error!("{msg}: Failed to write to file {:?}: {:?}", file_path, e);
            return Err(Box::new(e));
        }

        if let Err(e) = file.sync_all().await {
            error!("{msg}: Failed to sync file {:?}: {:?}", file_path, e);
            return Err(Box::new(e));
        }

        debug!("{msg}: Written successfully");
        Ok(())
    }
}
