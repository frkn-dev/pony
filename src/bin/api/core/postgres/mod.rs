use pony::ConnectionStorageApiOp;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use tokio_postgres::NoTls;

use pony::config::settings::PostgresConfig;
use pony::memory::node::Node;
use pony::Connection;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::OperationStatus;
use pony::Result;

use crate::core::postgres::connection::ConnRow;
use crate::core::postgres::connection::PgConn;
use crate::core::postgres::node::PgNode;
use crate::core::postgres::user::PgUser;
use crate::core::postgres::user::UserRow;

pub mod connection;
pub mod node;
pub mod user;

#[derive(Clone)]
pub struct PgContext {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgContext {
    pub async fn init(config: &PostgresConfig) -> Result<Self> {
        let connection_line = format!(
            "host={} user={} dbname={} password={} port={}",
            config.host, config.username, config.db, config.password, config.port
        );

        let (client, connection) = tokio_postgres::connect(&connection_line, NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Postgres connection error: {}", e);
            }
        });

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub fn node(&self) -> PgNode {
        PgNode::new(self.client.clone())
    }

    pub fn conn(&self) -> PgConn {
        PgConn::new(self.client.clone())
    }

    pub fn user(&self) -> PgUser {
        PgUser::new(self.client.clone())
    }
}

#[async_trait::async_trait]
pub trait Tasks {
    async fn add_node(&mut self, db_node: Node) -> Result<()>;
    async fn add_conn(&mut self, db_conn: ConnRow) -> Result<OperationStatus>;
    async fn add_user(&mut self, db_user: UserRow) -> Result<OperationStatus>;
}

#[async_trait::async_trait]
impl Tasks for MemoryCache<HashMap<String, Vec<Node>>, Connection> {
    async fn add_user(&mut self, db_user: UserRow) -> Result<OperationStatus> {
        let user_id = db_user.user_id;
        let user = db_user.try_into()?;

        if self.users.contains_key(&user_id) {
            return Ok(OperationStatus::AlreadyExist(user_id));
        }

        self.users.insert(user_id, user);
        Ok(OperationStatus::Ok(user_id))
    }

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
}
