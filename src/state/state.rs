use std::{
    collections::{hash_map::Entry, HashMap},
    error::Error,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    node::Node,
    stats::StatType,
    tag::Tag,
    user::{User, UserStatus},
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct State<T>
where
    T: Send + Sync + Clone + 'static,
{
    pub users: HashMap<Uuid, User>,
    pub nodes: T,
}

impl<T: Default> State<T>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    pub fn new() -> Self {
        State {
            users: HashMap::new(),
            nodes: T::default(),
        }
    }
}

impl State<Node> {
    pub fn with_node(node: Node) -> Self {
        Self {
            users: HashMap::new(),
            nodes: node,
        }
    }
}

pub trait NodeStorage {
    fn add_node(&mut self, new_node: Node) -> Result<(), Box<dyn Error>>;
    fn get_nodes(&self, env: Option<String>) -> Option<Vec<Node>>;
    fn get_node(&self) -> Option<Node>;
    fn get_node_by_uuid(&self, env: String, uuid: Option<Uuid>) -> Option<Node>;
    fn update_node_uplink(
        &mut self,
        tag: Tag,
        new_uplink: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>>;
    fn update_node_downlink(
        &mut self,
        tag: Tag,
        new_uplink: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>>;
    fn get_all_nodes(&self) -> Option<Vec<Node>>;
    fn update_node_user_count(
        &mut self,
        tag: Tag,
        user_count: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl NodeStorage for Node {
    fn add_node(&mut self, _new_node: Node) -> Result<(), Box<dyn Error>> {
        Err("Cannot add node to single Node instance".into())
    }

    fn get_nodes(&self, _env: Option<String>) -> Option<Vec<Node>> {
        Some(vec![self.clone()])
    }

    fn get_node(&self) -> Option<Node> {
        Some(self.clone())
    }

    fn get_node_by_uuid(&self, _env: String, _uuid: Option<Uuid>) -> Option<Node> {
        None
    }

    fn update_node_uplink(
        &mut self,
        tag: Tag,
        new_uplink: i64,
        _env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if self.uuid == node_id {
            self.update_uplink(tag, new_uplink)?;
            Ok(())
        } else {
            Err("Node ID does not match".into())
        }
    }

    fn update_node_downlink(
        &mut self,
        tag: Tag,
        new_downlink: i64,
        _env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if self.uuid == node_id {
            self.update_downlink(tag, new_downlink)?;
            Ok(())
        } else {
            Err("Node ID does not match".into())
        }
    }

    fn get_all_nodes(&self) -> Option<Vec<Node>> {
        Some(vec![self.clone()])
    }

    fn update_node_user_count(
        &mut self,
        tag: Tag,
        user_count: i64,
        _env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.uuid == node_id {
            self.update_user_count(tag, user_count)?;
            Ok(())
        } else {
            Err("Node ID does not match".into())
        }
    }
}

impl NodeStorage for HashMap<String, Vec<Node>> {
    fn add_node(&mut self, new_node: Node) -> Result<(), Box<dyn Error>> {
        let env = new_node.env.clone();
        let uuid = new_node.uuid;

        match self.get_mut(&env) {
            Some(nodes) => {
                if nodes.iter().any(|n| n.uuid == uuid) {
                    return Err(format!("Node {} already exists", uuid).into());
                }
                nodes.push(new_node);
            }
            None => {
                self.insert(env, vec![new_node]);
            }
        }

        Ok(())
    }

    fn get_node_by_uuid(&self, env: String, uuid: Option<Uuid>) -> Option<Node> {
        let nodes = self.get(&env)?;

        match uuid {
            Some(id) => nodes.iter().find(|n| n.uuid == id).cloned(),
            None => None,
        }
    }

    fn get_node(&self) -> Option<Node> {
        None
    }

    fn get_nodes(&self, env: Option<String>) -> Option<Vec<Node>> {
        match env {
            Some(env_key) => self.get(&env_key).cloned(),
            None => {
                let all_nodes: Vec<Node> = self.values().flat_map(|nodes| nodes.clone()).collect();
                if all_nodes.is_empty() {
                    None
                } else {
                    Some(all_nodes)
                }
            }
        }
    }

    fn update_node_uplink(
        &mut self,
        tag: Tag,
        new_uplink: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.get_mut(&env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == node_id) {
                node.update_uplink(tag, new_uplink)?;
                return Ok(());
            }
        }

        Err(format!("Node not found in env {} with id {}", env, node_id).into())
    }

    fn update_node_downlink(
        &mut self,
        tag: Tag,
        new_downlink: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.get_mut(&env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == node_id) {
                node.update_downlink(tag, new_downlink)?;
                return Ok(());
            }
        }

        Err(format!("Node not found in env {} with id {}", env, node_id).into())
    }

    fn get_all_nodes(&self) -> Option<Vec<Node>> {
        let nodes: Vec<Node> = self.values().flatten().cloned().collect();

        (!nodes.is_empty()).then_some(nodes)
    }

    fn update_node_user_count(
        &mut self,
        tag: Tag,
        user_count: i64,
        env: String,
        node_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(nodes) = self.get_mut(&env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == node_id) {
                node.update_user_count(tag, user_count)?;
                return Ok(());
            } else {
                return Err(format!("Node with ID {} not found", node_id).into());
            }
        }

        Err(format!("Environment '{}' not found", env).into())
    }
}

pub trait UserStorage {
    fn add_or_update_user(&mut self, user_id: Uuid, new_user: User) -> Result<(), Box<dyn Error>>;
    fn remove_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>>;
    fn restore_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>>;
    fn expire_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>>;
    fn get_user(&self, user_id: Uuid) -> Option<User>;
    fn update_user_limit(&mut self, user_id: Uuid, new_limit: i64) -> Result<(), Box<dyn Error>>;
    fn update_user_trial(&mut self, user_id: Uuid, new_trial: bool) -> Result<(), Box<dyn Error>>;
    fn update_user_stat(
        &mut self,
        user_id: Uuid,
        stat: StatType,
        value: Option<i64>,
    ) -> Result<(), Box<dyn Error>>;
    fn reset_user_stat(&mut self, user_id: Uuid, stat: StatType);
    fn get_all_trial_users(&self, status: UserStatus) -> HashMap<Uuid, User>;
    fn update_user_uplink(&mut self, user_id: Uuid, new_uplink: i64) -> Result<(), Box<dyn Error>>;
    fn update_user_downlink(
        &mut self,
        user_id: Uuid,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>>;
    fn update_user_online(&mut self, user_id: Uuid, new_online: i64) -> Result<(), Box<dyn Error>>;
}

impl<T> UserStorage for State<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn add_or_update_user(&mut self, user_id: Uuid, new_user: User) -> Result<(), Box<dyn Error>> {
        match self.users.entry(user_id) {
            Entry::Occupied(mut entry) => {
                let existing_user = entry.get_mut();
                existing_user.trial = new_user.trial;
                existing_user.limit = new_user.limit;
                existing_user.password = new_user.password;
                existing_user.status = new_user.status;
            }
            Entry::Vacant(entry) => {
                entry.insert(new_user);
            }
        }
        Ok(())
    }

    fn remove_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>> {
        self.users
            .remove(&user_id)
            .map(|_| ())
            .ok_or("User not found".into())
    }

    fn restore_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.status = UserStatus::Active;
            user.update_modified_at();
            Ok(())
        } else {
            Err("User not found".into())
        }
    }

    fn expire_user(&mut self, user_id: Uuid) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.status = UserStatus::Expired;
            user.update_modified_at();
            Ok(())
        } else {
            Err("User not found".into())
        }
    }

    fn get_user(&self, user_id: Uuid) -> Option<User> {
        self.users.get(&user_id).cloned()
    }

    fn update_user_limit(&mut self, user_id: Uuid, new_limit: i64) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.limit = new_limit;
            user.update_modified_at();
        }
        Ok(())
    }

    fn update_user_trial(&mut self, user_id: Uuid, new_trial: bool) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.trial = new_trial;
            user.update_modified_at();
        }
        Ok(())
    }

    fn update_user_stat(
        &mut self,
        user_id: Uuid,
        stat: StatType,
        new_value: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        let user = self.users.get_mut(&user_id).ok_or("User not found")?;
        match stat {
            StatType::Uplink => {
                user.uplink = Some(new_value.ok_or("Missing uplink value")?);
            }
            StatType::Downlink => {
                user.downlink = Some(new_value.ok_or("Missing downlink value")?);
            }
            StatType::Online => {
                user.online = Some(new_value.ok_or("Missing online value")?);
            }
        }
        user.update_modified_at();
        Ok(())
    }

    fn reset_user_stat(&mut self, user_id: Uuid, stat: StatType) {
        if let Some(user) = self.users.get_mut(&user_id) {
            match stat {
                StatType::Uplink => user.reset_uplink(),
                StatType::Downlink => user.reset_downlink(),
                StatType::Online => {} // Можно добавить user.reset_online(), если нужно
            }
        }
    }

    fn get_all_trial_users(&self, status: UserStatus) -> HashMap<Uuid, User> {
        self.users
            .iter()
            .filter(|(_, user)| user.status == status && user.trial)
            .map(|(user_id, user)| (*user_id, user.clone()))
            .collect()
    }

    fn update_user_uplink(&mut self, user_id: Uuid, new_uplink: i64) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.uplink = Some(new_uplink);
            user.update_modified_at();
            Ok(())
        } else {
            Err(format!("User not found: {}", user_id).into())
        }
    }

    fn update_user_downlink(
        &mut self,
        user_id: Uuid,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.downlink = Some(new_downlink);
            user.update_modified_at();
            Ok(())
        } else {
            Err(format!("User not found: {}", user_id).into())
        }
    }

    fn update_user_online(&mut self, user_id: Uuid, new_online: i64) -> Result<(), Box<dyn Error>> {
        if let Some(user) = self.users.get_mut(&user_id) {
            user.online = Some(new_online);
            user.update_modified_at();
            Ok(())
        } else {
            Err(format!("User not found: {}", user_id).into())
        }
    }
}
