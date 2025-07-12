use clickhouse::Client as ChClient;
use pony::metrics::metrics::Metric;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

pub mod query;
pub mod score;

#[derive(Clone)]
pub struct ChContext {
    url: String,
}

impl ChContext {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    pub fn client(&self) -> ChClient {
        ChClient::default().with_url(&self.url)
    }
}

impl ChContext {
    async fn execute_metric_query<T>(&self, query: &str) -> Option<Metric<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        log::debug!("Executing query:\n{}", query);
        let client = self.client();

        let result = client.query(query).fetch_all::<Metric<T>>().await;

        match &result {
            Ok(rows) => log::debug!("Fetched {} rows", rows.len()),
            Err(e) => log::error!("Query failed: {:?}", e),
        }

        result.ok().and_then(|mut rows| rows.pop())
    }
}
