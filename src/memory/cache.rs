use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::connection::op::api::Operations as ConnectionApiOp;
use super::connection::op::base::Operations as ConnectionBaseOp;
use super::node::Node;
use super::storage::node::Operations as NodeStorageOp;

#[derive(Archive, Deserialize, Serialize, RkyvDeserialize, RkyvSerialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Connections<C>(pub HashMap<uuid::Uuid, C>);

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
pub struct Cache<T, C>
where
    T: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
{
    pub connections: Connections<C>,
    pub nodes: T,
}

// COMMENT(qezz): This is basically `impl Default for State`
impl<T: Default, C> Cache<T, C>
where
    T: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp + ConnectionBaseOp + Clone + Send + Sync + 'static + PartialEq,
{
    pub fn new() -> Self {
        Cache {
            nodes: T::default(),
            connections: Connections::default(),
        }
    }
}

impl<C> Cache<Node, C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static + PartialEq,
{
    pub fn with_node(node: Node) -> Self {
        Self {
            nodes: node,
            connections: Connections::default(),
        }
    }
}
