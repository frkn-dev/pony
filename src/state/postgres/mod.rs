use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use tokio_postgres::NoTls;

use crate::config::settings::PostgresConfig;
use crate::state::postgres::connection::PgConn;
use crate::state::postgres::node::PgNode;
use crate::state::postgres::user::PgUser;
use crate::state::ConnRow;
use crate::state::SyncTask;
use crate::state::UserRow;
use crate::Result;

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

pub async fn run_shadow_sync(mut rx: mpsc::Receiver<SyncTask>, db: PgContext) {
    while let Some(task) = rx.recv().await {
        match task {
            SyncTask::InsertUser { user_id, user } => {
                let user_row: UserRow = (user_id, user).into();
                if let Err(err) = db.user().insert(user_row).await {
                    log::error!("Failed to sync InsertUser: {err}");
                }
            }
            SyncTask::UpdateUser { user_id, user } => {
                let user_row: UserRow = (user_id, user).into();
                if let Err(err) = db.user().update(user_row).await {
                    log::error!("Failed to sync UpdateUser: {err}");
                }
            }
            SyncTask::DeleteUser { user_id } => {
                if let Err(err) = db.user().delete(&user_id).await {
                    log::error!("Failed to sync DeleteUser: {err}");
                }
            }
            SyncTask::InsertNode { node_id, node } => {
                if let Err(err) = db.node().insert(node_id, node).await {
                    log::error!("Failed to sync InsertNode: {err}");
                }
            }
            SyncTask::UpdateNodeStatus {
                node_id,
                env,
                status,
            } => {
                if let Err(err) = db.node().update_status(&node_id, &env, status).await {
                    log::error!("Failed to sync UpdateNodeStatus: {err}");
                }
            }
            SyncTask::InsertConn { conn_id, conn } => {
                let conn_row: ConnRow = (conn_id, conn).into();
                if let Err(err) = db.conn().insert(conn_row).await {
                    log::error!("Failed to sync InsertConn: {err}");
                }
            }
            SyncTask::UpdateConn { conn_id, conn } => {
                let conn_row: ConnRow = (conn_id, conn).into();
                if let Err(err) = db.conn().update(conn_row).await {
                    log::error!("Failed to sync UpdateConn: {err}");
                }
            }
            SyncTask::DeleteConn { conn_id } => {
                if let Err(err) = db.conn().delete(&conn_id).await {
                    log::error!("Failed to sync DeleteConn: {err}");
                }
            }
            SyncTask::UpdateConnStat { conn_id, stat } => {
                if let Err(err) = db.conn().update_stat(&conn_id, stat).await {
                    log::error!("Failed to sync UpdateConnStat: {err}");
                }
            }
            SyncTask::UpdateConnStatus { conn_id, status } => {
                if let Err(err) = db.conn().update_status(&conn_id, status).await {
                    log::error!("Failed to sync UpdateConnStatus: {err}");
                }
            }
        }
    }
}
