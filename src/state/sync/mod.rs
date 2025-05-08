use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::state::connection::Conn;
use crate::state::connection::{ConnApiOp, ConnBaseOp};
use crate::state::node::Node;
use crate::state::node::NodeStatus;
use crate::state::storage::node::NodeStorage;
use crate::state::user::User;
use crate::state::ConnStat;
use crate::state::ConnStatus;
use crate::state::State;

mod tasks;

pub use tasks::SyncOp;

#[derive(Debug)]
pub enum SyncTask {
    InsertUser {
        user_id: uuid::Uuid,
        user: User,
    },
    InsertConn {
        conn_id: uuid::Uuid,
        conn: Conn,
    },
    UpdateConn {
        conn_id: uuid::Uuid,
        conn: Conn,
    },
    InsertNode {
        node_id: uuid::Uuid,
        node: Node,
    },
    UpdateNodeStatus {
        node_id: uuid::Uuid,
        env: String,
        status: NodeStatus,
    },
    UpdateConnStat {
        conn_id: uuid::Uuid,
        stat: ConnStat,
    },
    UpdateConnStatus {
        conn_id: uuid::Uuid,
        status: ConnStatus,
    },
}

#[derive(Clone)]
pub struct SyncState<N, C>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
{
    pub memory: Arc<Mutex<State<N, C>>>,
    pub sync_tx: mpsc::Sender<SyncTask>,
}

impl<N, C> SyncState<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Send + Sync + Clone + 'static + From<Conn>,
{
    pub fn new(memory: Arc<Mutex<State<N, C>>>, sync_tx: mpsc::Sender<SyncTask>) -> Self {
        Self { memory, sync_tx }
    }
}
