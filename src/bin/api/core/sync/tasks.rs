use pony::http::requests::ConnUpdateRequest;
use pony::http::requests::SubUpdateReq;
use pony::memory::cache::Connections;
use pony::memory::node::Node;
use pony::memory::node::Status as NodeStatus;
use pony::memory::subscription::Subscription;
use pony::Connection as Conn;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStorageApiOp as ApiOp;
use pony::ConnectionStorageBaseOp;
use pony::NodeStorageOp;
use pony::OperationStatus;
use pony::SubscriptionOp;
use pony::SubscriptionStorageOp;
use pony::SyncError;

use super::MemSync;
use crate::core::postgres::connection::ConnRow;

type SyncResult<T> = std::result::Result<T, SyncError>;

// Input validation traits
trait Validate {
    fn validate(&self) -> SyncResult<()>;
}

impl Validate for Node {
    fn validate(&self) -> SyncResult<()> {
        if self.env.trim().is_empty() {
            return Err(SyncError::Validation(
                "Environment cannot be empty".to_string(),
            ));
        }

        if self.env.len() > 50 {
            return Err(SyncError::Validation(
                "Environment name too long".to_string(),
            ));
        }

        Ok(())
    }
}

impl Validate for Conn {
    fn validate(&self) -> SyncResult<()> {
        // Add connection-specific validation

        Ok(())
    }
}

#[async_trait::async_trait]
pub trait SyncOp<N, C, S>
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
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> SyncResult<OperationStatus>;
    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> SyncResult<OperationStatus>;
    async fn add_sub(&self, sub: Subscription) -> SyncResult<OperationStatus>;
    async fn update_conn(
        &self,
        conn_id: &uuid::Uuid,
        conn: ConnUpdateRequest,
    ) -> SyncResult<OperationStatus>;
    async fn delete_connection(&self, conn_id: &uuid::Uuid) -> SyncResult<OperationStatus>;
    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> SyncResult<()>;
    async fn update_conn_stat(&self, conn_id: &uuid::Uuid, stat: ConnectionStat) -> SyncResult<()>;

    async fn update_sub(
        &self,
        sub_id: &uuid::Uuid,
        sub_req: SubUpdateReq,
    ) -> SyncResult<OperationStatus>;
}

#[async_trait::async_trait]
impl<N, C, S> SyncOp<N, C, S> for MemSync<N, C, S>
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
    S: SubscriptionOp
        + Send
        + Sync
        + Clone
        + 'static
        + PartialEq
        + std::convert::From<pony::Subscription>,
{
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> SyncResult<OperationStatus> {
        log::info!("Adding node: {}", node_id);

        // Validate input
        node.validate()?;

        // Insert into database first
        if let Err(e) = self.db.node().upsert(*node_id, node.clone()).await {
            log::error!("Failed to insert node {} into database: {}", node_id, e);
            return Err(SyncError::Database(e));
        }

        // Insert into memory
        let result = {
            let mut memory = self.memory.write().await;
            memory.nodes.add(node.clone())
        };

        match result {
            Ok(OperationStatus::Ok(_)) | Ok(OperationStatus::AlreadyExist(_)) => {
                log::info!("Successfully added node: {}", node_id);
                Ok(OperationStatus::Ok(*node_id))
            }
            Ok(OperationStatus::Updated(_)) => {
                log::info!("Successfully added node: {}", node_id);
                Ok(OperationStatus::Ok(*node_id))
            }
            Ok(OperationStatus::NotModified(id)) => {
                log::debug!("Node {} not modified", id);
                Ok(OperationStatus::NotModified(id))
            }
            Ok(other) => {
                log::error!(
                    "Unexpected operation status for node {}: {:?}",
                    node_id,
                    other
                );
                Err(SyncError::Memory(format!(
                    "Unexpected operation status: {:?}",
                    other
                )))
            }
            Err(e) => {
                log::error!("Memory error adding node {}: {}", node_id, e);
                Err(SyncError::Memory(e.to_string()))
            }
        }
    }

    async fn add_sub(&self, sub: Subscription) -> SyncResult<OperationStatus> {
        log::info!("Adding subscription: {}", sub.id);

        // Insert into database
        if let Err(e) = self.db.sub().create(&sub).await {
            log::error!(
                "Failed to insert subscription {} into database: {}",
                sub.id,
                e
            );
            return Err(SyncError::Database(e));
        }

        // Insert into memory
        let result = {
            let mut memory = self.memory.write().await;
            memory.subscriptions.add(sub.clone().into())
        };

        match result {
            OperationStatus::Ok(id) => {
                log::info!("Successfully added subscription: {}", id);
                Ok(OperationStatus::Ok(id))
            }
            OperationStatus::Updated(id) => {
                log::info!("Successfully updated subscription: {}", id);
                Ok(OperationStatus::Updated(id))
            }
            OperationStatus::AlreadyExist(id) => {
                log::info!("Subscription already exists: {}", id);
                Ok(OperationStatus::AlreadyExist(id))
            }
            OperationStatus::BadRequest(id, msg) => {
                log::warn!("Bad request for subscription {}: {}", id, msg);
                Ok(OperationStatus::BadRequest(id, msg))
            }
            other => {
                log::error!(
                    "Unexpected operation status for subscription {}: {:?}",
                    sub.id,
                    other
                );
                Err(SyncError::Memory(format!(
                    "Unexpected operation status: {:?}",
                    other
                )))
            }
        }
    }

    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> SyncResult<OperationStatus> {
        log::info!("Adding connection: {}", conn_id);

        // Validate input
        conn.validate()?;

        // Create database row
        let conn_row: ConnRow = (*conn_id, conn.clone().into()).into();

        // Insert into database first
        if let Err(e) = self.db.conn().insert(conn_row).await {
            log::error!(
                "Failed to insert connection {} into database: {}",
                conn_id,
                e
            );
            return Err(SyncError::Database(e));
        }

        // Insert into memory
        let result = {
            let mut memory = self.memory.write().await;
            ApiOp::add(&mut memory.connections, conn_id, conn.clone().into())
        };

        match result {
            Ok(OperationStatus::Ok(id)) | Ok(OperationStatus::AlreadyExist(id)) => {
                log::info!("Successfully added connection: {}", id);
                Ok(OperationStatus::Ok(id))
            }
            Ok(OperationStatus::BadRequest(id, msg)) => {
                log::warn!("Bad request for connection {}: {}", id, msg);
                Ok(OperationStatus::BadRequest(id, msg))
            }
            Ok(other) => {
                log::error!(
                    "Unexpected operation status for connection {}: {}",
                    conn_id,
                    other
                );
                Err(SyncError::Memory(format!(
                    "Unexpected operation status: {}",
                    other
                )))
            }
            Err(e) => {
                log::error!("Memory error adding connection {}: {}", conn_id, e);
                Err(SyncError::Memory(e.to_string()))
            }
        }
    }

    async fn update_conn(
        &self,
        conn_id: &uuid::Uuid,
        conn_req: ConnUpdateRequest,
    ) -> SyncResult<OperationStatus> {
        log::info!("Updating connection: {}", conn_id);

        // Get current connection
        let current_conn = {
            let memory = self.memory.read().await;
            memory.connections.get(conn_id)
        };

        let conn = match current_conn {
            Some(c) => c,
            None => {
                log::warn!("Connection {} not found for update", conn_id);
                return Ok(OperationStatus::NotFound(*conn_id));
            }
        };

        // Apply update
        let updated_conn =
            match <Connections<C> as ApiOp<C>>::apply_update(&mut conn.into(), conn_req) {
                Some(updated) => updated,
                None => {
                    log::debug!("No changes detected for connection: {}", conn_id);
                    return Ok(OperationStatus::NotModified(*conn_id));
                }
            };

        // Update database first
        let conn_row: ConnRow = (*conn_id, updated_conn.clone()).into();
        if let Err(e) = self.db.conn().update(conn_row).await {
            log::error!("Failed to update connection {} in database: {}", conn_id, e);
            return Err(SyncError::Database(e));
        }

        // Update memory
        {
            let mut memory = self.memory.write().await;
            memory.connections.insert(*conn_id, updated_conn.into());
        }

        log::info!("Successfully updated connection: {}", conn_id);
        Ok(OperationStatus::Updated(*conn_id))
    }

    async fn delete_connection(&self, conn_id: &uuid::Uuid) -> SyncResult<OperationStatus> {
        log::info!("Deleting connection: {}", conn_id);

        // Check if connection exists and get its current state
        let current_conn = {
            let memory = self.memory.read().await;
            memory.connections.get(conn_id)
        };

        let conn = match current_conn {
            Some(c) => c,
            None => {
                log::warn!("Connection {} not found for deletion", conn_id);
                return Ok(OperationStatus::NotFound(*conn_id));
            }
        };

        // Check if already deleted
        if conn.get_deleted() {
            return Ok(OperationStatus::NotModified(*conn_id));
        }

        // Delete from database first
        if let Err(e) = self.db.conn().delete(conn_id).await {
            log::error!(
                "Failed to delete connection {} from database: {}",
                conn_id,
                e
            );
            return Err(SyncError::Database(e));
        }

        // Mark as deleted in memory
        {
            let mut memory = self.memory.write().await;
            if let Some(conn_mut) = memory.connections.get_mut(conn_id) {
                conn_mut.set_deleted(true);
                conn_mut.set_modified_at();
            }
        }

        log::info!("Successfully deleted connection: {}", conn_id);
        Ok(OperationStatus::Ok(*conn_id))
    }

    async fn update_node_status(
        &self,
        uuid: &uuid::Uuid,
        env: &str,
        status: NodeStatus,
    ) -> SyncResult<()> {
        log::debug!(
            "Updating node status: {} in env {} to {:?}",
            uuid,
            env,
            status
        );

        // Get current node
        let current_node = {
            let memory = self.memory.read().await;
            memory.nodes.get(env, uuid).cloned()
        };

        let node = match current_node {
            Some(n) => n,
            None => {
                log::debug!("Node {} not found in env {}", uuid, env);
                return Ok(());
            }
        };

        // Check if status actually changed
        if node.status == status {
            log::debug!("Node {} status unchanged", uuid);
            return Ok(());
        }

        // Update database first
        if let Err(e) = self.db.node().update_status(uuid, env, status).await {
            log::error!("Failed to update node {} status in database: {}", uuid, e);
            return Err(SyncError::Database(e));
        }

        // Update memory
        {
            let mut memory = self.memory.write().await;
            if let Some(node_mut) = memory.nodes.get_mut(env, uuid) {
                if let Err(e) = node_mut.update_status(status) {
                    log::error!("Failed to update node {} status in memory: {}", uuid, e);
                    return Err(SyncError::Memory(e.to_string()));
                }
            }
        }

        log::debug!("Successfully updated node {} status", uuid);
        Ok(())
    }

    async fn update_conn_stat(
        &self,
        conn_id: &uuid::Uuid,
        conn_stat: ConnectionStat,
    ) -> SyncResult<()> {
        log::debug!("Updating connection stats: {}", conn_id);

        // Update database first
        if let Err(e) = self.db.conn().update_stat(conn_id, conn_stat.clone()).await {
            log::error!(
                "Failed to update connection {} stats in database: {}",
                conn_id,
                e
            );
            return Err(SyncError::Database(e));
        }

        {
            let mut memory = self.memory.write().await;
            if let Err(e) = memory.connections.update_stats(conn_id, conn_stat) {
                log::warn!(
                    "Failed to update connection {} stats in memory: {}",
                    conn_id,
                    e
                );
            }
        }

        log::debug!("Successfully updated connection {} stats", conn_id);
        Ok(())
    }

    async fn update_sub(
        &self,
        sub_id: &uuid::Uuid,
        sub_req: SubUpdateReq,
    ) -> SyncResult<OperationStatus> {
        log::info!("Updating subscription: {}", sub_id);

        let mut memory = self.memory.write().await;

        let sub = match memory.subscriptions.find_by_id_mut(sub_id) {
            Some(s) => s,
            None => {
                log::warn!("Subscription {} not found for update", sub_id);
                return Ok(OperationStatus::NotFound(*sub_id));
            }
        };

        if let Some(days) = sub_req.days {
            sub.extend(days);
        }

        if let Some(bonus_days) = sub_req.bonus_days {
            sub.set_bonus_days(bonus_days);
        }

        if let Some(ref_by) = sub_req.referred_by.clone() {
            sub.set_referred_by(ref_by);
        }

        if let Some(ref_code) = sub_req.refer_code.clone() {
            sub.set_refer_code(ref_code);
        }

        let expires_at = sub
            .expires_at()
            .ok_or_else(|| SyncError::InconsistentState {
                resource: "Subscription".to_string(),
                id: *sub_id,
            })?;

        if let Err(e) = self
            .db
            .sub()
            .update_subscription(
                *sub_id,
                expires_at,
                sub.bonus_days(),
                sub.referred_by().as_ref(),
                &sub.refer_code(),
            )
            .await
        {
            log::error!(
                "Failed to update subscription {} in database: {}",
                sub_id,
                e
            );
            return Err(SyncError::Database(e));
        }

        log::info!("Successfully updated subscription: {}", sub_id);
        Ok(OperationStatus::Updated(*sub_id))
    }
}
