use serde_json::json;
use std::collections::HashMap;

use crate::state::node::Node;
use crate::state::storage::Status as OperationStatus;
use crate::state::tag::Tag;
use crate::{PonyError, Result};

pub trait Operations {
    fn iter_nodes(&self) -> Box<dyn Iterator<Item = (&uuid::Uuid, &Node)> + '_>;
    fn add(&mut self, new_node: Node) -> Result<OperationStatus>;
    fn all(&self) -> Option<Vec<Node>>;
    fn all_json(&self) -> serde_json::Value;
    fn get_by_env(&self, env: &str) -> Option<Vec<Node>>;
    fn get_by_id(&self, id: &uuid::Uuid) -> Option<Node>;
    fn get(&self, env: &str, uuid: &uuid::Uuid) -> Option<&Node>;
    fn get_self(&self) -> Option<Node>;
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

impl Operations for Node {
    fn iter_nodes(&self) -> Box<dyn Iterator<Item = (&uuid::Uuid, &Node)> + '_> {
        Box::new(std::iter::empty())
    }
    fn add(&mut self, _new_node: Node) -> Result<OperationStatus> {
        Err(PonyError::Custom(
            "Cannot add node to single Node instance".into(),
        ))
    }
    fn get_by_env(&self, _env: &str) -> Option<Vec<Node>> {
        Some(vec![self.clone()])
    }
    fn all(&self) -> Option<Vec<Node>> {
        Some(vec![self.clone()])
    }
    fn all_json(&self) -> serde_json::Value {
        serde_json::to_value(vec![self]).unwrap_or_else(|_| serde_json::json!([]))
    }
    fn get_self(&self) -> Option<Node> {
        Some(self.clone())
    }
    fn get(&self, _env: &str, _uuid: &uuid::Uuid) -> Option<&Node> {
        None
    }

    fn get_mut(&mut self, _env: &str, _uuid: &uuid::Uuid) -> Option<&mut Node> {
        None
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
    fn get_by_id(&self, _: &uuid::Uuid) -> std::option::Option<Node> {
        None
    }
}

impl Operations for HashMap<String, Vec<Node>> {
    fn iter_nodes(&self) -> Box<dyn Iterator<Item = (&uuid::Uuid, &Node)> + '_> {
        let all_nodes: Vec<(&uuid::Uuid, &Node)> = self
            .values()
            .flat_map(|nodes_vec| nodes_vec.iter())
            .map(|node| (&node.uuid, node))
            .collect();

        Box::new(all_nodes.into_iter())
    }
    fn add(&mut self, new_node: Node) -> Result<OperationStatus> {
        let env = new_node.env.clone();
        let uuid = new_node.uuid;

        if self
            .values()
            .any(|nodes| nodes.iter().any(|n| n.uuid == uuid))
        {
            return Ok(OperationStatus::AlreadyExist(uuid));
        }

        match self.get_mut(&env) {
            Some(nodes) => {
                nodes.push(new_node);
            }
            None => {
                self.insert(env, vec![new_node]);
            }
        }

        Ok(OperationStatus::Ok(uuid))
    }

    fn get_self(&self) -> Option<Node> {
        None
    }
    fn get(&self, env: &str, uuid: &uuid::Uuid) -> Option<&Node> {
        self.get(env)?.iter().find(|n| &n.uuid == uuid)
    }
    fn get_mut(&mut self, env: &str, uuid: &uuid::Uuid) -> Option<&mut Node> {
        self.get_mut(env)?.iter_mut().find(|n| &n.uuid == uuid)
    }
    fn get_by_env(&self, env: &str) -> Option<Vec<Node>> {
        self.get(env)
            .map(|nodes| nodes.iter().cloned().collect::<Vec<_>>())
            .filter(|v| !v.is_empty())
    }
    fn get_by_id(&self, node_id: &uuid::Uuid) -> Option<Node> {
        self.values()
            .flat_map(|nodes| nodes.iter())
            .find(|node| &node.uuid == node_id)
            .cloned()
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
