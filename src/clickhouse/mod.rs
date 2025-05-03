use crate::state::StatType;
use clickhouse::Client as ChClient;
use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod query;

#[derive(Clone)]
pub struct ChContext {
    inner: Arc<Mutex<ChClient>>,
}

impl ChContext {
    pub fn new(url: &str) -> Self {
        let client = ChClient::default().with_url(url);

        Self {
            inner: Arc::new(Mutex::new(client)),
        }
    }

    pub fn client(&self) -> Arc<Mutex<ChClient>> {
        self.inner.clone()
    }
}

#[derive(Clone, Debug, Row, Serialize, Deserialize)]
pub struct MetricValue<T> {
    pub latest: i64,
    pub metric: String,
    pub value: T,
}

impl<T> MetricValue<T> {
    pub fn stat_type(&self) -> StatType {
        StatType::from_path(&self.metric)
    }
}
