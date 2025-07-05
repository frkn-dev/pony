use clickhouse::Client as ChClient;
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
