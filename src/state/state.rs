use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

use super::connection::ConnApiOp;
use super::connection::ConnBaseOp;
use super::node::Node;
use super::user::User;
use super::NodeStorage;

#[derive(Clone, Deserialize, Serialize, Debug)]
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
pub struct State<T, C>
where
    T: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
{
    pub users: HashMap<uuid::Uuid, User>,
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
            users: HashMap::default(),
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
            users: HashMap::default(),
            nodes: node,
            connections: Connections::default(),
        }
    }
}
