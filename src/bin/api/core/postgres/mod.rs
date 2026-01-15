use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use tokio_postgres::NoTls;

use pony::config::settings::PostgresConfig;
use pony::memory::node::Node;
use pony::Connection;
use pony::ConnectionStorageApiOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::OperationStatus;
use pony::Result;
use pony::Subscription;
use pony::SubscriptionStorageOp;

use crate::core::postgres::connection::ConnRow;
use crate::core::postgres::connection::PgConn;
use crate::core::postgres::node::PgNode;
use crate::core::postgres::subscription::PgSubscription;

pub mod connection;
pub mod node;
pub mod subscription;

pub struct PgClientManager {
    config: PostgresConfig,
    client: Option<PgClient>,
}

impl PgClientManager {
    pub async fn new(config: PostgresConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: None,
        })
    }

    async fn connect(&mut self) -> Result<()> {
        let connection_line = format!(
            "host={} user={} dbname={} password={} port={}",
            self.config.host,
            self.config.username,
            self.config.db,
            self.config.password,
            self.config.port
        );

        let (client, connection) = tokio_postgres::connect(&connection_line, NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                log::error!("Postgres connection dropped: {}", e);
            }
        });

        self.client = Some(client);
        Ok(())
    }

    pub async fn get_client(&mut self) -> Result<&mut PgClient> {
        if self.client.is_none() {
            self.connect().await?;
        }

        // ping with simple query
        let client = self.client.as_mut().unwrap();
        if let Err(e) = client.simple_query("SELECT 1").await {
            log::warn!("PG ping failed: {}. Reconnecting...", e);
            self.connect().await?;
        }

        Ok(self.client.as_mut().unwrap())
    }
}

#[derive(Clone)]
pub struct PgContext {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgContext {
    pub async fn init(config: &PostgresConfig) -> Result<Self> {
        let manager = PgClientManager::new(config.clone()).await?;
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    pub fn node(&self) -> PgNode {
        PgNode::new(self.manager.clone())
    }

    pub fn conn(&self) -> PgConn {
        PgConn::new(self.manager.clone())
    }

    pub fn sub(&self) -> PgSubscription {
        PgSubscription::new(self.manager.clone())
    }
}

#[async_trait::async_trait]
pub trait Tasks {
    async fn add_node(&mut self, db_node: Node) -> Result<()>;
    async fn add_conn(&mut self, db_conn: ConnRow) -> Result<OperationStatus>;
    async fn add_subscription(&mut self, db_sub: Subscription) -> OperationStatus;
}

#[async_trait::async_trait]
impl Tasks for MemoryCache<HashMap<String, Vec<Node>>, Connection, Subscription> {
    async fn add_conn(&mut self, db_conn: ConnRow) -> Result<OperationStatus> {
        let conn_id = db_conn.conn_id;
        let conn: Connection = db_conn.try_into()?;

        self.connections.add(&conn_id, conn.into()).map_err(|e| {
            format!(
                "Create: Failed to add connection {} to state: {}",
                conn_id, e
            )
            .into()
        })
    }
    async fn add_node(&mut self, db_node: Node) -> Result<()> {
        match self.nodes.add(db_node.clone()) {
            Ok(_) => {
                log::debug!("Node added to State: {}", db_node.uuid);
                Ok(())
            }
            Err(e) => Err(format!(
                "Create: Failed to add node {} to state: {}",
                db_node.uuid, e
            )
            .into()),
        }
    }

    async fn add_subscription(&mut self, db_sub: Subscription) -> OperationStatus {
        let id = db_sub.id;
        log::trace!("Processing subscription: {}", id);

        let status = self.subscriptions.add(db_sub);

        match &status {
            OperationStatus::Ok(_) => log::debug!("✓ Subscription {} stored", id),
            OperationStatus::Updated(_) => log::debug!("↻ Subscription {} updated", id),
            OperationStatus::AlreadyExist(_) => log::debug!("○ Subscription {} unchanged", id),
            _ => log::debug!("Not implemented {}", id),
        }

        status
    }
}
