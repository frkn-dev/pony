use chrono::Utc;
use pony::http::requests::ConnUpdateRequest;
use pony::http::requests::UserUpdateReq;
use pony::memory::cache::Connections;
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
use pony::SyncError;
use pony::UserStorageOp;

use super::MemSync;
use crate::core::postgres::connection::ConnRow;
use crate::core::postgres::user::UserRow;

type SyncResult<T> = std::result::Result<T, SyncError>;

// Input validation traits
trait Validate {
    fn validate(&self) -> SyncResult<()>;
}

impl Validate for User {
    fn validate(&self) -> SyncResult<()> {
        if self.username.trim().is_empty() {
            return Err(SyncError::Validation(
                "Username cannot be empty".to_string(),
            ));
        }

        if self.username.len() > 255 {
            return Err(SyncError::Validation("Username too long".to_string()));
        }

        if let Some(ref password) = self.password {
            if password.len() < 8 {
                return Err(SyncError::Validation("Password too short".to_string()));
            }
        }

        Ok(())
    }
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
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> SyncResult<OperationStatus>;
    async fn update_user(
        &self,
        user_id: &uuid::Uuid,
        user: UserUpdateReq,
    ) -> SyncResult<OperationStatus>;
    async fn delete_user(&self, user_id: &uuid::Uuid) -> SyncResult<OperationStatus>;
    async fn add_node(&self, node_id: &uuid::Uuid, node: Node) -> SyncResult<OperationStatus>;
    async fn add_conn(&self, conn_id: &uuid::Uuid, conn: Conn) -> SyncResult<OperationStatus>;
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
    async fn activate_trial_conn(&self, conn_id: &uuid::Uuid) -> SyncResult<OperationStatus>;
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
    async fn add_user(&self, user_id: &uuid::Uuid, user: User) -> SyncResult<OperationStatus> {
        log::info!("Adding user: {}", user_id);

        // Validate input
        user.validate()?;

        // Check for existing user in memory first (with username uniqueness check)
        {
            let memory = self.memory.read().await;
            if let Some((existing_id, existing_user)) = memory
                .users
                .iter()
                .find(|(_, u)| u.username == user.username)
            {
                if existing_user.is_deleted {
                    log::debug!("User {} was previously deleted", existing_id);
                    return Ok(OperationStatus::DeletedPreviously(*existing_id));
                } else {
                    log::debug!("User {} already exists", existing_id);
                    return Ok(OperationStatus::AlreadyExist(*existing_id));
                }
            }
        }

        // Create database row
        let user_row: UserRow = (*user_id, user.clone()).into();

        // Insert into database first

        self.db.user().insert(user_row).await.map_err(|e| {
            log::error!("Failed to insert user {} into database: {}", user_id, e);
            e
        })?;
        // Insert into memory
        {
            let mut memory = self.memory.write().await;

            // Double-check in case another thread inserted while we were waiting
            if let Some((existing_id, existing_user)) = memory
                .users
                .iter()
                .find(|(_, u)| u.username == user.username)
            {
                if existing_user.is_deleted {
                    return Ok(OperationStatus::DeletedPreviously(*existing_id));
                } else {
                    return Ok(OperationStatus::AlreadyExist(*existing_id));
                }
            }

            memory.users.insert(*user_id, user);
        }

        log::info!("Successfully added user: {}", user_id);
        Ok(OperationStatus::Ok(*user_id))
    }

    async fn update_user(
        &self,
        user_id: &uuid::Uuid,
        user_req: UserUpdateReq,
    ) -> SyncResult<OperationStatus> {
        log::info!("Updating user: {}", user_id);

        // Get current user state
        let current_user = {
            let memory = self.memory.read().await;
            memory.users.get(user_id).cloned()
        };

        let mut user = match current_user {
            Some(u) => u,
            None => {
                log::warn!("User {} not found for update", user_id);
                return Ok(OperationStatus::NotFound(*user_id));
            }
        };

        // Check if user is deleted and we're not explicitly undeleting
        if user_req.is_deleted.is_none() && user.is_deleted {
            return Ok(OperationStatus::NotFound(*user_id));
        }

        // Apply changes and track if anything changed
        let mut changed = false;
        let _original_user = user.clone();

        if let Some(is_deleted) = user_req.is_deleted {
            if user.is_deleted != is_deleted {
                user.is_deleted = is_deleted;
                changed = true;
            }
        }

        if let Some(env) = user_req.env {
            if user.env != env {
                user.env = env;
                changed = true;
            }
        }

        if let Some(password) = user_req.password {
            if user.password.as_ref() != Some(&password) {
                user.password = Some(password);
                changed = true;
            }
        }

        if let Some(limit) = user_req.limit {
            if user.limit != Some(limit) {
                user.limit = Some(limit);
                changed = true;
            }
        }

        if !changed {
            log::debug!("No changes detected for user: {}", user_id);
            return Ok(OperationStatus::NotModified(*user_id));
        }

        // Validate updated user
        user.validate()?;

        // Update timestamp
        user.modified_at = Utc::now().naive_utc();

        // Update database first
        if let Err(e) = self.db.user().update(user_id, user.clone()).await {
            log::error!("Failed to update user {} in database: {}", user_id, e);
            return Err(SyncError::Database(e));
        }

        // Update memory
        {
            let mut memory = self.memory.write().await;

            // Verify user still exists in memory
            if !memory.users.contains_key(user_id) {
                log::warn!("User {} disappeared from memory during update", user_id);
                return Ok(OperationStatus::NotFound(*user_id));
            }

            memory.users.insert(*user_id, user);
        }

        log::info!("Successfully updated user: {}", user_id);
        Ok(OperationStatus::Updated(*user_id))
    }

    async fn delete_user(&self, user_id: &uuid::Uuid) -> SyncResult<OperationStatus> {
        log::info!("Deleting user: {}", user_id);

        // Check if user exists in memory
        let exists_in_memory = {
            let memory = self.memory.read().await;
            memory.users.contains_key(user_id)
        };

        if !exists_in_memory {
            log::warn!("User {} not found for deletion", user_id);
            return Ok(OperationStatus::NotFound(*user_id));
        }

        // Delete from database first
        if let Err(e) = self.db.user().delete(user_id).await {
            log::error!("Failed to delete user {} from database: {}", user_id, e);
            return Err(SyncError::Database(e));
        }

        // Delete from memory
        let status = {
            let mut memory = self.memory.write().await;
            memory.users.delete(user_id)
        };

        log::info!("Successfully deleted user: {}", user_id);
        Ok(status)
    }

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
            return Ok(OperationStatus::NotFound(*conn_id));
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

        // Update memory
        {
            let mut memory = self.memory.write().await;
            if let Err(e) = memory.connections.update_stats(conn_id, conn_stat) {
                log::warn!(
                    "Failed to update connection {} stats in memory: {}",
                    conn_id,
                    e
                );
                // Don't return error here as database is already updated
            }
        }

        log::debug!("Successfully updated connection {} stats", conn_id);
        Ok(())
    }

    async fn activate_trial_conn(&self, conn_id: &uuid::Uuid) -> SyncResult<OperationStatus> {
        log::info!("Activating trial connection: {}", conn_id);

        // Get current connection
        let current_conn = {
            let memory = self.memory.read().await;
            memory.connections.get(conn_id)
        };

        let mut conn = match current_conn {
            Some(c) => c,
            None => {
                log::error!("Connection {} not found for activation", conn_id);
                return Err(SyncError::ResourceNotFound {
                    resource: "connection".to_string(),
                    id: *conn_id,
                });
            }
        };

        let result = match conn.get_status() {
            ConnectionStatus::Expired => {
                // Reset stats and activate
                if let Err(e) = self
                    .db
                    .conn()
                    .update_stat(conn_id, ConnectionStat::default())
                    .await
                {
                    log::error!("Failed to reset connection {} stats: {}", conn_id, e);
                    return Err(SyncError::Database(e));
                }

                if let Err(e) = self
                    .db
                    .conn()
                    .update_status(conn_id, ConnectionStatus::Active)
                    .await
                {
                    log::error!("Failed to activate connection {}: {}", conn_id, e);
                    return Err(SyncError::Database(e));
                }

                // Update memory
                conn.set_uplink(0);
                conn.set_downlink(0);
                conn.set_status(ConnectionStatus::Active);
                conn.set_modified_at();

                {
                    let mut memory = self.memory.write().await;
                    memory.connections.insert(*conn_id, conn);
                }

                OperationStatus::Updated(*conn_id)
            }
            ConnectionStatus::Active => {
                // Just reset stats
                if let Err(e) = self
                    .db
                    .conn()
                    .update_stat(conn_id, ConnectionStat::default())
                    .await
                {
                    log::error!("Failed to reset connection {} stats: {}", conn_id, e);
                    return Err(SyncError::Database(e));
                }

                // Update memory
                {
                    let mut memory = self.memory.write().await;
                    if let Some(c) = memory.connections.get_mut(conn_id) {
                        c.set_uplink(0);
                        c.set_downlink(0);
                        c.set_modified_at();
                    }
                }

                OperationStatus::UpdatedStat(*conn_id)
            }
        };

        log::info!("Successfully activated trial connection: {}", conn_id);
        Ok(result)
    }
}
