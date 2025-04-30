use std::collections::HashMap;

use crate::state::connection::Conn;
use crate::state::connection::ConnBase;
use crate::state::node::Node;
use crate::state::state::State;

pub mod connection;
pub mod node;
pub mod state;
pub mod stats;
pub mod tag;

pub type ApiState = State<HashMap<String, Vec<Node>>, Conn>;
pub type AgentState = State<Node, ConnBase>;
