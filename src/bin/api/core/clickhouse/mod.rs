use clickhouse::Client as ChClient;

pub mod query;

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
