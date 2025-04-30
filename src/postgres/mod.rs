use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use tokio_postgres::NoTls;

use crate::config::settings::PostgresConfig;
use crate::postgres::connection::PgConn;
use crate::postgres::node::PgNode;
use crate::postgres::user::PgUser;

pub mod connection;
pub mod node;
pub mod user;

pub async fn postgres_client(
    pg_settings: PostgresConfig,
) -> Result<Arc<Mutex<PgClient>>, Box<dyn Error>> {
    let connection_line = format!(
        "host={} user={} dbname={} password={} port={}",
        pg_settings.host,
        pg_settings.username,
        pg_settings.db,
        pg_settings.password,
        pg_settings.port
    );

    let (client, connection) = tokio_postgres::connect(&connection_line, NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Error in PostgreSQL connection: {}", e);
        }
    });

    Ok(Arc::new(Mutex::new(client)))
}

#[derive(Clone)]
pub struct PgContext {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgContext {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
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
