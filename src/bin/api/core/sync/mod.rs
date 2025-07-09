use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use pony::memory::node::Node;
use pony::memory::user::User;
use pony::Connection as Conn;
use pony::MemoryCache;

use pony::memory::node::Status as NodeStatus;
use pony::ConnectionStat;
use pony::ConnectionStatus;

use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;

pub(crate) mod tasks;

#[derive(Debug)]
pub enum SyncTask {
    InsertUser {
        user_id: uuid::Uuid,
        user: User,
    },
    DeleteUser {
        user_id: uuid::Uuid,
    },
    UpdateUser {
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
    DeleteConn {
        conn_id: uuid::Uuid,
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
        stat: ConnectionStat,
    },
    UpdateConnStatus {
        conn_id: uuid::Uuid,
        status: ConnectionStatus,
    },
}

#[derive(Clone)]
pub struct MemSync<N, C>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<MemoryCache<N, C>>>,
    pub sync_tx: mpsc::Sender<SyncTask>,
}

impl<N, C> MemSync<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + From<Conn> + PartialEq,
{
    pub fn new(memory: Arc<RwLock<MemoryCache<N, C>>>, sync_tx: mpsc::Sender<SyncTask>) -> Self {
        Self { memory, sync_tx }
    }
}
