use pony::http::requests::ConnUpdateRequest;
use pony::http::requests::UserUpdateReq;
use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::memory::user::User;
use pony::Connection as Conn;
use pony::ConnectionApiOp;

use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::ConnectionStorageApiOp as ApiOp;
use pony::ConnectionStorageBaseOp;
use pony::NodeStorageOp;
use pony::OperationStatus;
use pony::UserStorageOp;
use pony::{PonyError, Result};

use super::MemSync;
use super::SyncTask;

#[async_trait::async_trait]
pub trait SyncOp<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp
        + ConnectionApiOp
        + Send
        + Sync
        + Clone
        + 'static
        + From<Conn>
        + std::cmp::PartialEq,
{
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> Result<OperationStatus>;
    async fn update_user(
        &self,
        user_id: &uuid::Uuid,
        user: UserUpdateReq,
    ) -> Result<OperationStatus>;
    async fn delete_user(&self, user_id: &uuid::Uuid) -> Result<OperationStatus>;
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> Result<OperationStatus>;
    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> Result<OperationStatus>;
    async fn update_conn(
        &self,
        conn_id: &uuid::Uuid,
        conn: ConnUpdateRequest,
    ) -> Result<OperationStatus>;
    async fn delete_connection(&self, conn_id: &uuid::Uuid) -> Result<OperationStatus>;
    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> Result<()>;
    async fn update_conn_stat(&self, conn_id: &uuid::Uuid, stat: ConnectionStat) -> Result<()>;
    async fn activate_trial_conn(&self, conn_id: &uuid::Uuid) -> Result<OperationStatus>;
}

#[async_trait::async_trait]
impl<N, C> SyncOp<N, C> for MemSync<N, C>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp
        + ConnectionApiOp
        + Send
        + Sync
        + Clone
        + 'static
        + From<Conn>
        + std::cmp::PartialEq,
    Conn: From<C>,
{
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;

        if let Some((id, u)) = mem.users.iter().find(|(_, u)| u.username == user.username) {
            if u.is_deleted {
                return Ok(OperationStatus::DeletedPreviously(*id));
            } else {
                return Ok(OperationStatus::AlreadyExist(*id));
            }
        }

        mem.users.insert(*user_id, user.clone());

        self.sync_tx
            .send(SyncTask::InsertUser {
                user_id: *user_id,
                user,
            })
            .await?;

        Ok(OperationStatus::Ok(*user_id))
    }

    async fn update_user(
        &self,
        user_id: &uuid::Uuid,
        user_upd: UserUpdateReq,
    ) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;

        if !mem.users.contains_key(user_id) {
            return Ok(OperationStatus::NotFound(*user_id));
        }

        match mem.users.update(user_id, user_upd.clone()) {
            OperationStatus::NotModified(id) => return Ok(OperationStatus::NotModified(id)),
            OperationStatus::Updated(id) => {
                if let Some(user) = mem.users.get(user_id) {
                    self.sync_tx
                        .send(SyncTask::UpdateUser {
                            user_id: *user_id,
                            user: user.clone(),
                        })
                        .await?;
                    Ok(OperationStatus::Updated(id))
                } else {
                    Ok(OperationStatus::NotFound(id))
                }
            }
            OperationStatus::NotFound(id) => Ok(OperationStatus::NotFound(id)),
            _ => unreachable!(),
        }
    }

    async fn delete_user(&self, user_id: &uuid::Uuid) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;

        match mem.users.delete(user_id) {
            OperationStatus::Ok(id) => {
                self.sync_tx
                    .send(SyncTask::DeleteUser { user_id: id })
                    .await?;
                Ok(OperationStatus::Ok(id))
            }
            OperationStatus::NotFound(id) => Ok(OperationStatus::NotFound(id)),
            _ => Err(PonyError::Custom("Op doesn't supported".into())),
        }
    }

    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;

        match mem.nodes.add(node.clone()) {
            Ok(OperationStatus::Ok(_)) => {
                self.sync_tx
                    .send(SyncTask::InsertNode {
                        node_id: *node_id,
                        node: node,
                    })
                    .await?;
                Ok(OperationStatus::Ok(*node_id))
            }
            Ok(OperationStatus::NotModified(id)) | Ok(OperationStatus::AlreadyExist(id)) => {
                Ok(OperationStatus::AlreadyExist(id))
            }
            Ok(OperationStatus::Updated(_))
            | Ok(OperationStatus::DeletedPreviously(_))
            | Ok(OperationStatus::BadRequest(_, _))
            | Ok(OperationStatus::UpdatedStat(_))
            | Ok(OperationStatus::NotFound(_)) => {
                Err(PonyError::Custom(format!("Operation doesn't supported")).into())
            }
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }

    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;
        match ApiOp::add(&mut mem.connections, conn_id, conn.clone().into()) {
            Ok(OperationStatus::Ok(id)) => {
                self.sync_tx
                    .send(SyncTask::InsertConn {
                        conn_id: *conn_id,
                        conn: conn.clone(),
                    })
                    .await?;

                Ok(OperationStatus::Ok(id))
            }
            Ok(OperationStatus::BadRequest(id, msg)) => Ok(OperationStatus::BadRequest(id, msg)),
            Ok(op) => Err(PonyError::Custom(format!("Operation isn't supported {}", op)).into()),
            Err(e) => Err(PonyError::Custom(format!("{}", e)).into()),
        }
    }
    async fn update_conn(
        &self,
        conn_id: &uuid::Uuid,
        conn_req: ConnUpdateRequest,
    ) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;
        match ApiOp::update(&mut mem.connections, conn_id, conn_req) {
            OperationStatus::Updated(id) => {
                if let Some(conn) = mem.connections.get(conn_id) {
                    self.sync_tx
                        .send(SyncTask::UpdateConn {
                            conn_id: *conn_id,
                            conn: conn.into(),
                        })
                        .await?;

                    Ok(OperationStatus::Updated(id))
                } else {
                    Ok(OperationStatus::NotFound(id))
                }
            }
            OperationStatus::Ok(id) => Ok(OperationStatus::Ok(id)),
            OperationStatus::NotModified(id) => Ok(OperationStatus::NotModified(id)),
            OperationStatus::BadRequest(id, msg) => Ok(OperationStatus::BadRequest(id, msg)),
            OperationStatus::NotFound(id) => Ok(OperationStatus::NotFound(id)),
            _ => Ok(OperationStatus::BadRequest(
                *conn_id,
                "Operation is not supported".into(),
            )),
        }
    }
    async fn delete_connection(&self, conn_id: &uuid::Uuid) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;

        match mem.connections.delete(conn_id) {
            Ok(_) => {
                self.sync_tx
                    .send(SyncTask::DeleteConn { conn_id: *conn_id })
                    .await?;
                Ok(OperationStatus::Ok(*conn_id))
            }
            Err(_) => Ok(OperationStatus::NotFound(*conn_id)),
        }
    }
    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> Result<()> {
        let mut mem = self.memory.write().await;
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

    async fn update_conn_stat(&self, uuid: &uuid::Uuid, conn_stat: ConnectionStat) -> Result<()> {
        let mut mem = self.memory.write().await;

        let _ = mem.connections.update_stats(uuid, conn_stat.clone());

        self.sync_tx
            .send(SyncTask::UpdateConnStat {
                conn_id: *uuid,
                stat: conn_stat,
            })
            .await?;

        Ok(())
    }

    async fn activate_trial_conn(&self, conn_id: &uuid::Uuid) -> Result<OperationStatus> {
        let mut mem = self.memory.write().await;

        if let Some(conn) = mem.connections.get_mut(&conn_id) {
            if conn.get_status() == ConnectionStatus::Expired {
                let _uplink = conn.set_uplink(0);
                let _downlink = conn.set_downlink(0);

                conn.set_status(ConnectionStatus::Active);
                conn.set_modified_at();

                let _ = self
                    .sync_tx
                    .send(SyncTask::UpdateConnStat {
                        conn_id: *conn_id,
                        stat: ConnectionStat::default(),
                    })
                    .await;

                let _ = self
                    .sync_tx
                    .send(SyncTask::UpdateConnStatus {
                        conn_id: *conn_id,
                        status: ConnectionStatus::Active,
                    })
                    .await;

                return Ok(OperationStatus::Updated(*conn_id));
            } else if conn.get_status() == ConnectionStatus::Active {
                let _uplink = conn.set_uplink(0);
                let _downlink = conn.set_downlink(0);
                conn.set_modified_at();

                let _ = self
                    .sync_tx
                    .send(SyncTask::UpdateConnStat {
                        conn_id: *conn_id,
                        stat: ConnectionStat::default(),
                    })
                    .await;
                return Ok(OperationStatus::UpdatedStat(*conn_id));
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
