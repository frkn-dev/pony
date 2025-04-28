use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;

use crate::postgres::connection::PgConn;
use crate::postgres::node::PgNode;

pub mod connection;
pub mod node;
pub mod postgres;

#[derive(Clone)]
pub struct DbContext {
    pub client: Arc<Mutex<PgClient>>,
}

impl DbContext {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub fn node(&self) -> PgNode {
        PgNode::new(self.client.clone())
    }

    pub fn conn(&self) -> PgConn {
        PgConn::new(self.client.clone())
    }
}
