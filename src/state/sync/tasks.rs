use crate::state::connection::Conn;
use crate::state::connection::ConnApiOp;
use crate::state::connection::ConnBaseOp;
use crate::state::node::NodeStatus;
use crate::state::storage::connection::ConnStorageOpStatus;
use crate::state::storage::node::NodeStorage;
use crate::state::storage::node::NodeStorageOpStatus;
use crate::state::storage::user::UserStorageOpStatus;
use crate::state::sync::SyncState;
use crate::state::sync::SyncTask;
use crate::state::user::User;
use crate::state::ConnStorageApi;
use crate::state::Node;
use crate::state::UserStorage;
use crate::{PonyError, Result};

#[async_trait::async_trait]
pub trait SyncOp<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Send + Sync + Clone + 'static + From<Conn>,
{
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> Result<UserStorageOpStatus>;
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> Result<NodeStorageOpStatus>;
    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> Result<ConnStorageOpStatus>;
    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> Result<()>;
}

#[async_trait::async_trait]
impl<N, C> SyncOp<N, C> for SyncState<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Send + Sync + Clone + 'static + From<Conn>,
{
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> Result<UserStorageOpStatus> {
        let mut mem = self.memory.lock().await;

        match mem.users.try_add(*user_id, user.clone()) {
            Ok(UserStorageOpStatus::Ok) => {
                self.sync_tx
                    .send(SyncTask::InsertUser {
                        user_id: *user_id,
                        user: user,
                    })
                    .await?;
                Ok(UserStorageOpStatus::Ok)
            }
            Ok(UserStorageOpStatus::AlreadyExist) => Ok(UserStorageOpStatus::AlreadyExist),
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> Result<NodeStorageOpStatus> {
        let mut mem = self.memory.lock().await;

        match mem.nodes.add(node.clone()) {
            Ok(NodeStorageOpStatus::Ok) => {
                self.sync_tx
                    .send(SyncTask::InsertNode {
                        node_id: *node_id,
                        node: node,
                    })
                    .await?;
                Ok(NodeStorageOpStatus::Ok)
            }
            Ok(NodeStorageOpStatus::AlreadyExist) => Ok(NodeStorageOpStatus::AlreadyExist),
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }

    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> Result<ConnStorageOpStatus> {
        let mut mem = self.memory.lock().await;
        match mem.connections.add_or_update(conn_id, conn.clone().into()) {
            Ok(ConnStorageOpStatus::Ok) => {
                self.sync_tx
                    .send(SyncTask::InsertConn {
                        conn_id: *conn_id,
                        conn: conn.clone(),
                    })
                    .await?;

                Ok(ConnStorageOpStatus::Ok)
            }
            Ok(ConnStorageOpStatus::AlreadyExist) => Ok(ConnStorageOpStatus::AlreadyExist),
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }
    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> Result<()> {
        let mut mem = self.memory.lock().await;
        if let Some(node) = mem.nodes.get_mut(env, uuid) {
            if node.status != status {
                node.update_status(status)?;
                self.sync_tx
                    .send(SyncTask::UpdateNodeStatus {
                        uuid: *uuid,
                        env: env.to_string(),
                        status,
                    })
                    .await?;
            }
        }
        Ok(())
    }
}
