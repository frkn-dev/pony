use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::error::Error;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::connection::Conn;
use super::connection::ConnStatus;
use super::node::Node;
use super::stats::StatType;
use super::tag::Tag;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct State<T>
where
    T: Send + Sync + Clone + 'static,
{
    pub users: HashSet<uuid::Uuid>,
    pub connections: HashMap<uuid::Uuid, Conn>,
    pub nodes: T,
}

impl<T: Default> State<T>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    pub fn new() -> Self {
        State {
            users: HashSet::default(),
            nodes: T::default(),
            connections: HashMap::default(),
        }
    }
}

impl State<Node> {
    pub fn with_node(node: Node) -> Self {
        Self {
            users: HashSet::default(),
            nodes: node,
            connections: HashMap::default(),
        }
    }
}

pub trait NodeStorage {
    fn add(&mut self, new_node: Node) -> Result<(), Box<dyn Error>>;
    fn all(&self) -> Option<Vec<Node>>;
    fn all_json(&self) -> serde_json::Value;
    fn by_env(&self, env: &str) -> Option<Vec<Node>>;
    fn get(&self) -> Option<Node>;
    fn get_mut(&mut self, env: &str, uuid: &uuid::Uuid) -> Option<&mut Node>;
    fn update_node_uplink(
        &mut self,
        tag: &Tag,
        new_uplink: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error>>;
    fn update_node_downlink(
        &mut self,
        tag: &Tag,
        new_uplink: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error>>;
    fn update_node_conn_count(
        &mut self,
        tag: &Tag,
        node_count: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl NodeStorage for Node {
    fn add(&mut self, _new_node: Node) -> Result<(), Box<dyn Error>> {
        Err("Cannot add node to single Node instance".into())
    }
    fn by_env(&self, _env: &str) -> Option<Vec<Node>> {
        Some(vec![self.clone()])
    }
    fn all(&self) -> Option<Vec<Node>> {
        Some(vec![self.clone()])
    }
    fn all_json(&self) -> serde_json::Value {
        serde_json::to_value(vec![self]).unwrap_or_else(|_| serde_json::json!([]))
    }
    fn get(&self) -> Option<Node> {
        Some(self.clone())
    }
    fn get_mut(&mut self, env: &str, uuid: &uuid::Uuid) -> Option<&mut Node> {
        if &self.uuid == uuid && self.env == env {
            Some(self)
        } else {
            None
        }
    }

    fn update_node_uplink(
        &mut self,
        tag: &Tag,
        new_uplink: i64,
        _env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if &self.uuid == node_id {
            self.update_uplink(tag, new_uplink)?;
            Ok(())
        } else {
            Err("Node ID does not match".into())
        }
    }

    fn update_node_downlink(
        &mut self,
        tag: &Tag,
        new_downlink: i64,
        _env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if self.uuid == *node_id {
            self.update_downlink(tag, new_downlink)?;
            Ok(())
        } else {
            Err("Node ID does not match".into())
        }
    }

    fn update_node_conn_count(
        &mut self,
        tag: &Tag,
        conn_count: i64,
        _env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.uuid == *node_id {
            self.update_conn_count(tag, conn_count)?;
            Ok(())
        } else {
            Err("Node ID does not match".into())
        }
    }
}

impl NodeStorage for HashMap<String, Vec<Node>> {
    fn add(&mut self, new_node: Node) -> Result<(), Box<dyn Error>> {
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

    fn get(&self) -> Option<Node> {
        None
    }
    fn get_mut(&mut self, env: &str, uuid: &uuid::Uuid) -> Option<&mut Node> {
        self.get_mut(env)?.iter_mut().find(|n| &n.uuid == uuid)
    }

    fn by_env(&self, env: &str) -> Option<Vec<Node>> {
        self.get(env).cloned()
    }
    fn all(&self) -> Option<Vec<Node>> {
        let nodes: Vec<Node> = self.values().flatten().cloned().collect();

        (!nodes.is_empty()).then_some(nodes)
    }

    fn all_json(&self) -> serde_json::Value {
        let nodes: Vec<&Node> = self.values().flat_map(|v| v.iter()).collect();
        serde_json::to_value(&nodes).unwrap_or_else(|_| json!([]))
    }

    fn update_node_uplink(
        &mut self,
        tag: &Tag,
        new_uplink: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.get_mut(env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == *node_id) {
                node.update_uplink(tag, new_uplink)?;
                return Ok(());
            }
        }
        Err(format!("Node not found in env {} with id {}", env, node_id).into())
    }

    fn update_node_downlink(
        &mut self,
        tag: &Tag,
        new_downlink: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(nodes) = self.get_mut(env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == *node_id) {
                node.update_downlink(tag, new_downlink)?;
                return Ok(());
            }
        }
        Err(format!("Node not found in env {} with id {}", env, node_id).into())
    }

    fn update_node_conn_count(
        &mut self,
        tag: &Tag,
        conn_count: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(nodes) = self.get_mut(env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == *node_id) {
                node.update_conn_count(tag, conn_count)?;
                return Ok(());
            } else {
                return Err(format!("Node with ID {} not found", node_id).into());
            }
        }
        Err(format!("Environment '{}' not found", env).into())
    }
}

pub trait ConnStorage {
    fn add_or_update(&mut self, conn_id: &uuid::Uuid, new_conn: Conn)
        -> Result<(), Box<dyn Error>>;
    fn remove(&mut self, conn_id: &uuid::Uuid) -> Result<(), Box<dyn Error>>;
    fn restore(&mut self, conn_id: &uuid::Uuid) -> Result<(), Box<dyn Error>>;
    fn expire(&mut self, conn_id: &uuid::Uuid) -> Result<(), Box<dyn Error>>;
    fn get(&self, conn_id: &uuid::Uuid) -> Option<Conn>;
    fn update_limit(&mut self, conn_id: &uuid::Uuid, new_limit: i64) -> Result<(), Box<dyn Error>>;
    fn update_trial(&mut self, conn_id: &uuid::Uuid, new_trial: bool)
        -> Result<(), Box<dyn Error>>;
    fn update_stat(
        &mut self,
        conn_id: &uuid::Uuid,
        stat: StatType,
        value: Option<i64>,
    ) -> Result<(), Box<dyn Error>>;
    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType);
    fn all_trial(&self, status: ConnStatus) -> HashMap<uuid::Uuid, Conn>;
    fn update_uplink(
        &mut self,
        conn_id: &uuid::Uuid,
        new_uplink: i64,
    ) -> Result<(), Box<dyn Error>>;
    fn update_downlink(
        &mut self,
        conn_id: &uuid::Uuid,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>>;
    fn update_online(
        &mut self,
        conn_id: &uuid::Uuid,
        new_online: i64,
    ) -> Result<(), Box<dyn Error>>;
}

impl ConnStorage for HashMap<uuid::Uuid, Conn> {
    fn add_or_update(
        &mut self,
        conn_id: &uuid::Uuid,
        new_conn: Conn,
    ) -> Result<(), Box<dyn Error>> {
        match self.entry(*conn_id) {
            Entry::Occupied(mut entry) => {
                let existing_conn = entry.get_mut();
                existing_conn.trial = new_conn.trial;
                existing_conn.limit = new_conn.limit;
                existing_conn.password = new_conn.password;
                existing_conn.status = new_conn.status;
            }
            Entry::Vacant(entry) => {
                entry.insert(new_conn);
            }
        }
        Ok(())
    }

    fn remove(&mut self, conn_id: &uuid::Uuid) -> Result<(), Box<dyn Error>> {
        self.remove(conn_id)
            .map(|_| ())
            .ok_or("Conn not found".into())
    }

    fn restore(&mut self, conn_id: &uuid::Uuid) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.status = ConnStatus::Active;
            conn.update_modified_at();
            Ok(())
        } else {
            Err("Conn not found".into())
        }
    }

    fn expire(&mut self, conn_id: &uuid::Uuid) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(&conn_id) {
            conn.status = ConnStatus::Expired;
            conn.update_modified_at();
            Ok(())
        } else {
            Err("Conn not found".into())
        }
    }

    fn get(&self, conn_id: &uuid::Uuid) -> Option<Conn> {
        self.get(conn_id).cloned()
    }

    fn update_limit(&mut self, conn_id: &uuid::Uuid, new_limit: i64) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.limit = new_limit;
            conn.update_modified_at();
        }
        Ok(())
    }

    fn update_trial(
        &mut self,
        conn_id: &uuid::Uuid,
        new_trial: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.trial = new_trial;
            conn.update_modified_at();
        }
        Ok(())
    }

    fn update_stat(
        &mut self,
        conn_id: &uuid::Uuid,
        stat: StatType,
        new_value: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        let conn = self.get_mut(&conn_id).ok_or("Conn not found")?;
        match stat {
            StatType::Uplink => {
                conn.uplink = Some(new_value.ok_or("Missing uplink value")?);
            }
            StatType::Downlink => {
                conn.downlink = Some(new_value.ok_or("Missing downlink value")?);
            }
            StatType::Online => {
                conn.online = Some(new_value.ok_or("Missing online value")?);
            }
        }
        conn.update_modified_at();
        Ok(())
    }

    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType) {
        if let Some(conn) = self.get_mut(conn_id) {
            match stat {
                StatType::Uplink => conn.reset_uplink(),
                StatType::Downlink => conn.reset_downlink(),
                StatType::Online => {}
            }
        }
    }

    fn all_trial(&self, status: ConnStatus) -> HashMap<uuid::Uuid, Conn> {
        self.iter()
            .filter(|(_, conn)| conn.status == status && conn.trial)
            .map(|(conn_id, conn)| (*conn_id, conn.clone()))
            .collect()
    }

    fn update_uplink(
        &mut self,
        conn_id: &uuid::Uuid,
        new_uplink: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.uplink = Some(new_uplink);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(format!("Conn not found: {}", conn_id).into())
        }
    }

    fn update_downlink(
        &mut self,
        conn_id: &uuid::Uuid,
        new_downlink: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.downlink = Some(new_downlink);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(format!("Conn not found: {}", conn_id).into())
        }
    }

    fn update_online(
        &mut self,
        conn_id: &uuid::Uuid,
        new_online: i64,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.online = Some(new_online);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(format!("Conn not found: {}", conn_id).into())
        }
    }
}
