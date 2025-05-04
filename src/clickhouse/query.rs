use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use uuid::Uuid;

use super::ChContext;
use super::MetricValue;

#[async_trait]
pub trait Queries {
    async fn fetch_node_heartbeat<T>(
        &self,
        env: &str,
        uuid: &Uuid,
        hostname: &str,
    ) -> Option<MetricValue<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static;

    async fn fetch_conn_stats<T>(
        &self,
        conn_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> Option<Vec<MetricValue<T>>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static;
}

#[async_trait]
impl Queries for ChContext {
    async fn fetch_node_heartbeat<T>(
        &self,
        env: &str,
        uuid: &Uuid,
        hostname: &str,
    ) -> Option<MetricValue<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        let query = format!(
            "SELECT 
                toInt64(toUnixTimestamp(toDateTime(anyLast(Timestamp)))) AS latest,
                Path AS metric,
                toFloat64(anyLast(Value)) AS value
            FROM default.graphite_data
            WHERE Path LIKE '{}.{}.{}.heartbeat'
            GROUP BY Path",
            env, hostname, uuid
        );

        let client = self.client();
        let client = client.lock().await;

        let result = client.query(&query).fetch_all::<MetricValue<T>>().await;

        result.ok().and_then(|mut rows| rows.pop())
    }

    async fn fetch_conn_stats<T>(
        &self,
        conn_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> Option<Vec<MetricValue<T>>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        let start_str = start.format("%Y-%m-%d %H:%M:%S").to_string();
        let query = format!(
            "SELECT 
                    extract(Path, '[^.]+$') AS metric,
                    toFloat64(sum(Value)) AS value
            FROM default.graphite_data
            WHERE Path LIKE '%.%.{conn_id}.conn_stat.%'
                  AND Timestamp >= toDateTime({start})
                  AND Timestamp < toDateTime({start}) + INTERVAL 1 DAY
            GROUP BY metric",
            conn_id = conn_id,
            start = start_str,
        );

        let client = self.client();
        let client = client.lock().await;

        let result = client.query(&query).fetch_all::<MetricValue<T>>().await;

        result.ok()
    }
}
