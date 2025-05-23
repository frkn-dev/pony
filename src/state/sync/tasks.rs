use crate::state::connection::Conn;
use crate::state::connection::ConnApiOp;
use crate::state::connection::ConnBaseOp;
use crate::state::node::NodeStatus;
use crate::state::storage::connection::ConnStorageBase;
use crate::state::storage::connection::ConnStorageOpStatus;
use crate::state::storage::node::NodeStorage;
use crate::state::storage::node::NodeStorageOpStatus;
use crate::state::storage::user::UserStorageOpStatus;
use crate::state::sync::SyncState;
use crate::state::sync::SyncTask;
use crate::state::user::User;
use crate::state::ConnStat;
use crate::state::ConnStatus;
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
    async fn delete_user(&self, user_id: &uuid::Uuid) -> Result<()>;
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> Result<NodeStorageOpStatus>;
    async fn add_or_update_conn(
        &self,
        conn_id: &uuid::Uuid,
        conn: Conn,
    ) -> Result<ConnStorageOpStatus>;
    async fn delete_connection(&self, conn_id: &uuid::Uuid) -> Result<()>;
    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> Result<()>;
    async fn update_conn_stat(&self, conn_id: &uuid::Uuid, stat: ConnStat) -> Result<()>;
    async fn activate_trial_conn(&self, conn_id: &uuid::Uuid) -> Result<ConnStorageOpStatus>;
}

#[async_trait::async_trait]
impl<N, C> SyncOp<N, C> for SyncState<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Send + Sync + Clone + 'static + From<Conn>,
{
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> Result<UserStorageOpStatus> {
        let mut mem = self.memory.lock().await;

        match mem.users.try_add(user_id, user.clone()) {
            Ok(UserStorageOpStatus::Ok) => {
                self.sync_tx
                    .send(SyncTask::InsertUser {
                        user_id: *user_id,
                        user: user,
                    })
                    .await?;
                Ok(UserStorageOpStatus::Ok)
            }
            Ok(UserStorageOpStatus::Updated) => {
                self.sync_tx
                    .send(SyncTask::UpdateUser {
                        user_id: *user_id,
                        user: user,
                    })
                    .await?;
                Ok(UserStorageOpStatus::Updated)
            }

            Ok(UserStorageOpStatus::AlreadyExist) => Ok(UserStorageOpStatus::AlreadyExist),
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }

    async fn delete_user(&self, user_id: &uuid::Uuid) -> Result<()> {
        let mut mem = self.memory.lock().await;

        match mem.users.delete(user_id) {
            Ok(_) => {
                self.sync_tx
                    .send(SyncTask::DeleteUser { user_id: *user_id })
                    .await?;
                Ok(())
            }
            Err(e) => Err(PonyError::Custom(e.to_string())),
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

    async fn add_or_update_conn(
        &self,
        conn_id: &uuid::Uuid,
        conn: Conn,
    ) -> Result<ConnStorageOpStatus> {
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
            Ok(ConnStorageOpStatus::Updated) => {
                self.sync_tx
                    .send(SyncTask::UpdateConn {
                        conn_id: *conn_id,
                        conn: conn.clone(),
                    })
                    .await?;

                Ok(ConnStorageOpStatus::Updated)
            }
            Ok(op) => Err(PonyError::Custom(format!("Operation isn't supported {}", op)).into()),
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }
    async fn delete_connection(&self, conn_id: &uuid::Uuid) -> Result<()> {
        let mut mem = self.memory.lock().await;

        match mem.connections.delete(conn_id) {
            Ok(_) => {
                self.sync_tx
                    .send(SyncTask::DeleteConn { conn_id: *conn_id })
                    .await?;
                Ok(())
            }
            Err(e) => Err(PonyError::Custom(e.to_string())),
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
                        node_id: *uuid,
                        env: env.to_string(),
                        status,
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn update_conn_stat(&self, uuid: &uuid::Uuid, conn_stat: ConnStat) -> Result<()> {
        let mut mem = self.memory.lock().await;

        let _ = mem.connections.update_stats(uuid, conn_stat.clone());

        self.sync_tx
            .send(SyncTask::UpdateConnStat {
                conn_id: *uuid,
                stat: conn_stat,
            })
            .await?;

        Ok(())
    }

    async fn activate_trial_conn(&self, conn_id: &uuid::Uuid) -> Result<ConnStorageOpStatus> {
        let mut mem = self.memory.lock().await;

        if let Some(conn) = mem.connections.get_mut(&conn_id) {
            if conn.get_status() == ConnStatus::Expired {
                let _uplink = conn.set_uplink(0);
                let _downlink = conn.set_downlink(0);

                conn.set_status(ConnStatus::Active);
                conn.set_modified_at();

                let _ = self
                    .sync_tx
                    .send(SyncTask::UpdateConnStat {
                        conn_id: *conn_id,
                        stat: ConnStat::default(),
                    })
                    .await;

                let _ = self
                    .sync_tx
                    .send(SyncTask::UpdateConnStatus {
                        conn_id: *conn_id,
                        status: ConnStatus::Active,
                    })
                    .await;

                return Ok(ConnStorageOpStatus::Updated);
            } else if conn.get_status() == ConnStatus::Active {
                let _uplink = conn.set_uplink(0);
                let _downlink = conn.set_downlink(0);
                conn.set_modified_at();

                let _ = self
                    .sync_tx
                    .send(SyncTask::UpdateConnStat {
                        conn_id: *conn_id,
                        stat: ConnStat::default(),
                    })
                    .await;
                return Ok(ConnStorageOpStatus::UpdatedStat);
            } else {
                return Err(PonyError::Custom(
                    format!("Status {} not found ", conn.get_status()).into(),
                ));
            }
        } else {
            return Err(PonyError::Custom(
                format!("Connection {} not found ", conn_id).into(),
            ));
        }
    }
}
