use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;

use crate::postgres::node::PgNodeRequest;
use crate::postgres::subscription::PgSubscriptionRequest;
use crate::postgres::user::PgUserRequest;

pub mod node;
pub mod postgres;
pub mod subscription;
pub mod user;

#[derive(Clone)]
pub struct DbContext {
    pub client: Arc<Mutex<PgClient>>,
}

impl DbContext {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub fn node(&self) -> PgNodeRequest {
        PgNodeRequest::new(self.client.clone())
    }

    pub fn user(&self) -> PgUserRequest {
        PgUserRequest::new(self.client.clone())
    }

    pub fn subscription(&self) -> PgSubscriptionRequest {
        PgSubscriptionRequest::new(self.client.clone())
    }
}
