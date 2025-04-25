use std::sync::Arc;

use clickhouse::Client;
use clickhouse::Row;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;

pub mod query;

#[derive(Clone)]
pub struct ChContext {
    inner: Arc<Mutex<Client>>,
}

impl ChContext {
    pub fn new(url: &str) -> Self {
        let client = Client::default().with_url(url);

        Self {
            inner: Arc::new(Mutex::new(client)),
        }
    }

    pub fn client(&self) -> Arc<Mutex<Client>> {
        self.inner.clone()
    }
}

#[derive(Clone, Debug, Row, Serialize, Deserialize)]
pub struct MetricValue<T> {
    pub latest: i64,
    pub metric: String,
    pub value: T,
}
