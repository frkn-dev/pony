use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    error::Error,
};
use uuid::Uuid;

use super::{
    node::Node,
    stats::StatType,
    tag::Tag,
    user::{User, UserStatus},
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct State {
    pub users: HashMap<Uuid, User>,
    pub nodes: HashMap<String, Vec<Node>>,
}

impl State {
    pub fn new() -> Self {
        State {
            users: HashMap::new(),
            nodes: HashMap::new(),
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
                debug!("Adding new user: {:?} {:?}", user_id, new_user);
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
            debug!("User {} removed  ", user_id);
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
                StatType::Online => {
                    if let Some(value) = new_value {
                        user.online = Some(value);
                    } else {
                        return Err("New value for Online is None".into());
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

    pub async fn update_user_online(
        &mut self,
        user_id: Uuid,
        new_online: i64,
    ) -> Result<(), Box<dyn Error>> {
        self.update_user_stat(user_id, StatType::Online, Some(new_online))
            .await
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
                StatType::Online => {}
            }
        } else {
            error!("User not found: {}", user_id);
        }
    }

    pub fn get_all_trial_users(&self, status: UserStatus) -> HashMap<Uuid, User> {
        self.users
            .iter()
            .filter(|(_, user)| user.status == status && user.trial)
            .map(|(user_id, user)| (user_id.clone(), user.clone()))
            .collect()
    }

    pub async fn add_node(&mut self, new_node: Node) -> Result<(), Box<dyn Error>> {
        let env = new_node.env.clone();
        let uuid = new_node.uuid.clone();
        debug!("Starting add_node for node in env {}: {:?}", env, new_node);

        debug!("Current nodes: {:?}", self.nodes);

        if let Some(existing_nodes) = self.nodes.get_mut(&env) {
            if existing_nodes.iter().any(|node| node.uuid == uuid) {
                debug!(
                    "Node with uuid {} already exists in env {}, not adding",
                    uuid, env
                );
                return Err(format!("node {} alresy exist", new_node.uuid).into());
            }
        }

        match self.nodes.entry(env.clone()) {
            Entry::Occupied(mut entry) => {
                debug!("Environment {} already exists, appending new node", env);
                entry.get_mut().push(new_node);
            }
            Entry::Vacant(entry) => {
                debug!("Adding new node list for environment {}", env);
                entry.insert(vec![new_node]);
            }
        }

        Ok(())
    }

    pub fn get_node(&self, env: String, uuid: Uuid) -> Option<Node> {
        self.nodes
            .get(&env)
            .and_then(|nodes| nodes.iter().find(|node| node.uuid == uuid))
            .cloned()
    }

    pub fn get_nodes(&self, env: String) -> Option<Vec<Node>> {
        if let Some(nodes) = self.nodes.get(&env).cloned() {
            Some(nodes)
        } else {
            None
        }
    }

    pub fn get_all_nodes(&self) -> Option<Vec<Node>> {
        let nodes: Vec<_> = self
            .nodes
            .values()
            .flat_map(|nodes| nodes.clone())
            .collect();

        if nodes.is_empty() {
            return None;
        }
        Some(nodes)
    }

    pub async fn update_node_uplink(
        &mut self,
        tag: Tag,
        new_uplink: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.nodes.get_mut(&env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == node_id) {
                node.update_uplink(tag, new_uplink)?;
            } else {
                error!("Node not found: {}", node_id);
                return Err(format!("Node with ID {} not found", node_id).into());
            }
        } else {
            error!("Environment not found: {}", env);
            return Err(format!("Environment '{}' not found", env).into());
        }

        Ok(())
    }

    pub async fn update_node_downlink(
        &mut self,
        tag: Tag,
        new_downlink: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.nodes.get_mut(&env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == node_id) {
                node.update_downlink(tag, new_downlink)?;
            } else {
                error!("Node not found: {}", node_id);
                return Err(format!("Node with ID {} not found", node_id).into());
            }
        } else {
            error!("Environment not found: {}", env);
            return Err(format!("Environment '{}' not found", env).into());
        }

        Ok(())
    }

    pub async fn update_node_user_count(
        &mut self,
        tag: Tag,
        user_count: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.nodes.get_mut(&env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == node_id) {
                node.update_user_count(tag, user_count)?;
            } else {
                error!("Node not found: {}", node_id);
                return Err(format!("Node with ID {} not found", node_id).into());
            }
        } else {
            error!("Environment not found: {}", env);
            return Err(format!("Environment '{}' not found", env).into());
        }

        Ok(())
    }
}
