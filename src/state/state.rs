use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::fmt;
use std::ops::{Deref, DerefMut};

use super::connection::ConnApiOp;
use super::connection::ConnBaseOp;
use super::connection::ConnStatus;
use super::node::Node;
use super::stats::StatType;
use super::tag::Tag;
use crate::{PonyError, Result};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Connections<C>(HashMap<uuid::Uuid, C>);

impl<C> Default for Connections<C> {
    fn default() -> Self {
        Connections(HashMap::new())
    }
}

impl<C: fmt::Display> fmt::Display for Connections<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (uuid, conn) in &self.0 {
            writeln!(f, "{} => {}", uuid, conn)?;
        }
        Ok(())
    }
}

impl<C> Deref for Connections<C> {
    type Target = HashMap<uuid::Uuid, C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C> DerefMut for Connections<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct State<T, C>
where
    T: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
{
    pub users: HashSet<uuid::Uuid>,
    pub connections: Connections<C>,
    pub nodes: T,
}

impl<T: Default, C> State<T, C>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        State {
            users: HashSet::default(),
            nodes: T::default(),
            connections: Connections::default(),
        }
    }
}

impl<C> State<Node, C>
where
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub fn with_node(node: Node) -> Self {
        Self {
            users: HashSet::default(),
            nodes: node,
            connections: Connections::default(),
        }
    }
}

pub trait NodeStorage {
    fn add(&mut self, new_node: Node) -> Result<()>;
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
    ) -> Result<()>;
    fn update_node_downlink(
        &mut self,
        tag: &Tag,
        new_uplink: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<()>;
    fn update_node_conn_count(
        &mut self,
        tag: &Tag,
        node_count: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<()>;
}

impl NodeStorage for Node {
    fn add(&mut self, _new_node: Node) -> Result<()> {
        Err(PonyError::Custom(
            "Cannot add node to single Node instance".into(),
        ))
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
    ) -> Result<()> {
        if &self.uuid == node_id {
            self.update_uplink(tag, new_uplink)?;
            Ok(())
        } else {
            Err(PonyError::Custom("Node ID does not match".into()))
        }
    }

    fn update_node_downlink(
        &mut self,
        tag: &Tag,
        new_downlink: i64,
        _env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<()> {
        if self.uuid == *node_id {
            self.update_downlink(tag, new_downlink)?;
            Ok(())
        } else {
            Err(PonyError::Custom("Node ID does not match".into()))
        }
    }

    fn update_node_conn_count(
        &mut self,
        tag: &Tag,
        conn_count: i64,
        _env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<()> {
        if self.uuid == *node_id {
            self.update_conn_count(tag, conn_count)?;
            Ok(())
        } else {
            Err(PonyError::Custom("Node ID does not match".into()))
        }
    }
}

impl NodeStorage for HashMap<String, Vec<Node>> {
    fn add(&mut self, new_node: Node) -> Result<()> {
        let env = new_node.env.clone();
        let uuid = new_node.uuid;

        match self.get_mut(&env) {
            Some(nodes) => {
                if nodes.iter().any(|n| n.uuid == uuid) {
                    return Err(PonyError::Custom(
                        format!("Node {} already exists", uuid).into(),
                    ));
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
        let mut result = Vec::new();

        if let Some(nodes) = self.get(env) {
            result.extend_from_slice(nodes);
        }

        if let Some(all_nodes) = self.get("all") {
            result.extend_from_slice(all_nodes);
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
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
    ) -> Result<()> {
        if let Some(nodes) = self.get_mut(env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == *node_id) {
                node.update_uplink(tag, new_uplink)?;
                return Ok(());
            }
        }
        Err(PonyError::Custom(
            format!("Node not found in env {} with id {}", env, node_id).into(),
        ))
    }

    fn update_node_downlink(
        &mut self,
        tag: &Tag,
        new_downlink: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<()> {
        if let Some(nodes) = self.get_mut(env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == *node_id) {
                node.update_downlink(tag, new_downlink)?;
                return Ok(());
            }
        }
        Err(PonyError::Custom(
            format!("Node not found in env {} with id {}", env, node_id).into(),
        ))
    }

    fn update_node_conn_count(
        &mut self,
        tag: &Tag,
        conn_count: i64,
        env: &str,
        node_id: &uuid::Uuid,
    ) -> Result<()> {
        if let Some(nodes) = self.get_mut(env) {
            if let Some(node) = nodes.iter_mut().find(|n| n.uuid == *node_id) {
                node.update_conn_count(tag, conn_count)?;
                return Ok(());
            } else {
                return Err(PonyError::Custom(
                    format!("Node with ID {} not found", node_id).into(),
                ));
            }
        }
        Err(PonyError::Custom(
            format!("Environment '{}' not found", env).into(),
        ))
    }
}

pub trait ConnStorageApi<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn add_or_update(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<()>;
    fn restore(&mut self, conn_id: &uuid::Uuid) -> Result<()>;
    fn expire(&mut self, conn_id: &uuid::Uuid) -> Result<()>;
    fn update_limit(&mut self, conn_id: &uuid::Uuid, new_limit: i32) -> Result<()>;
    fn update_trial(&mut self, conn_id: &uuid::Uuid, new_trial: bool) -> Result<()>;
    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType);
    fn all_trial(&self, status: ConnStatus) -> HashMap<uuid::Uuid, C>;
}

pub trait ConnStorageBase<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn add(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<()>;
    fn remove(&mut self, conn_id: &uuid::Uuid) -> Result<()>;
    fn get(&self, conn_id: &uuid::Uuid) -> Option<C>;
    fn get_by_user_id(&self, user_id: &uuid::Uuid) -> Option<Vec<(uuid::Uuid, C)>>;
    fn update_stat(
        &mut self,
        conn_id: &uuid::Uuid,
        stat: StatType,
        value: Option<i64>,
    ) -> Result<()>;
    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType);
    fn update_uplink(&mut self, conn_id: &uuid::Uuid, new_uplink: i64) -> Result<()>;
    fn update_downlink(&mut self, conn_id: &uuid::Uuid, new_downlink: i64) -> Result<()>;
    fn update_online(&mut self, conn_id: &uuid::Uuid, new_online: i64) -> Result<()>;
}

impl<C> ConnStorageBase<C> for Connections<C>
where
    C: ConnBaseOp + Clone + Send + Sync + 'static,
{
    fn add(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<()> {
        match self.entry(*conn_id) {
            Entry::Occupied(_) => {
                return Err(PonyError::Custom("Connection already exist".into()));
            }
            Entry::Vacant(entry) => {
                entry.insert(new_conn);
            }
        }
        Ok(())
    }

    fn remove(&mut self, conn_id: &uuid::Uuid) -> Result<()> {
        self.0
            .remove(conn_id)
            .map(|_| ())
            .ok_or(PonyError::Custom("Conn not found".into()))
    }

    fn get(&self, conn_id: &uuid::Uuid) -> Option<C> {
        self.0.get(conn_id).cloned()
    }

    fn get_by_user_id(&self, user_id: &uuid::Uuid) -> Option<Vec<(uuid::Uuid, C)>> {
        let conns: Vec<(uuid::Uuid, C)> = self
            .iter()
            .filter(|(_, conn)| conn.get_user_id() == Some(*user_id))
            .map(|(conn_id, conn)| (*conn_id, conn.clone()))
            .collect();

        if conns.is_empty() {
            None
        } else {
            Some(conns)
        }
    }

    fn update_stat(
        &mut self,
        conn_id: &uuid::Uuid,
        stat: StatType,
        new_value: Option<i64>,
    ) -> Result<()> {
        let conn = self
            .get_mut(&conn_id)
            .ok_or(PonyError::Custom("Conn not found".into()))?;
        match stat {
            StatType::Uplink => {
                conn.set_uplink(new_value.unwrap_or(0));
            }
            StatType::Downlink => {
                conn.set_downlink(new_value.unwrap_or(0));
            }
            StatType::Online => {
                conn.set_online(new_value.unwrap_or(0));
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

    fn update_uplink(&mut self, conn_id: &uuid::Uuid, new_uplink: i64) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_uplink(new_uplink);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom(
                format!("Conn not found: {}", conn_id).into(),
            ))
        }
    }

    fn update_downlink(&mut self, conn_id: &uuid::Uuid, new_downlink: i64) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_downlink(new_downlink);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom(
                format!("Conn not found: {}", conn_id).into(),
            ))
        }
    }

    fn update_online(&mut self, conn_id: &uuid::Uuid, new_online: i64) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_online(new_online);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom(
                format!("Conn not found: {}", conn_id).into(),
            ))
        }
    }
}

impl<C> ConnStorageApi<C> for Connections<C>
where
    C: ConnBaseOp + ConnApiOp + Clone + Send + Sync + 'static,
{
    fn add_or_update(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<()> {
        match self.entry(*conn_id) {
            Entry::Occupied(mut entry) => {
                let existing_conn = entry.get_mut();
                existing_conn.set_trial(new_conn.get_trial());
                existing_conn.set_limit(new_conn.get_limit());
                existing_conn.set_password(new_conn.get_password());
                existing_conn.set_status(new_conn.get_status());
            }
            Entry::Vacant(entry) => {
                entry.insert(new_conn);
            }
        }
        Ok(())
    }

    fn restore(&mut self, conn_id: &uuid::Uuid) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_status(ConnStatus::Active);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom("Conn not found".into()))
        }
    }

    fn expire(&mut self, conn_id: &uuid::Uuid) -> Result<()> {
        if let Some(conn) = self.get_mut(&conn_id) {
            conn.set_status(ConnStatus::Expired);
            conn.update_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom("Conn not found".into()))
        }
    }

    fn update_limit(&mut self, conn_id: &uuid::Uuid, new_limit: i32) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_limit(new_limit);
            conn.update_modified_at();
        }
        Ok(())
    }

    fn update_trial(&mut self, conn_id: &uuid::Uuid, new_trial: bool) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_trial(new_trial);
            conn.update_modified_at();
        }
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

    fn all_trial(&self, status: ConnStatus) -> HashMap<uuid::Uuid, C> {
        self.iter()
            .filter(|(_, conn)| conn.get_status() == status && conn.get_trial())
            .map(|(conn_id, conn)| (*conn_id, conn.clone()))
            .collect()
    }
}
