use std::fmt::Debug;

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::ChContext;
use super::MetricValue;

#[async_trait]
pub trait Queries {
    async fn fetch_node_heartbeat<T>(&self, env: &str, uuid: Uuid) -> Option<MetricValue<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static;

    async fn fetch_user_uplink_traffic<T>(
        &self,
        env: &str,
        user_id: uuid::Uuid,
        modified_at: DateTime<Utc>,
    ) -> Option<MetricValue<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static;
}

#[async_trait]
impl Queries for ChContext {
    async fn fetch_node_heartbeat<T>(&self, env: &str, uuid: Uuid) -> Option<MetricValue<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        let query = format!(
            "SELECT 
                toInt64(toUnixTimestamp(toDateTime(anyLast(Timestamp)))) AS latest,
                Path AS metric,
                toFloat64(anyLast(Value)) AS value
            FROM default.graphite_data
            WHERE Path LIKE '{}.{}.heartbeat'
            GROUP BY Path",
            env, uuid
        );

        let client = self.client();
        let client = client.lock().await;

        let result = client.query(&query).fetch_all::<MetricValue<T>>().await;

        result.ok().and_then(|mut rows| rows.pop())
    }

    async fn fetch_user_uplink_traffic<T>(
        &self,
        env: &str,
        user_id: uuid::Uuid,
        modified_at: DateTime<Utc>,
    ) -> Option<MetricValue<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        let modified_at_str = modified_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let query = format!(
            "SELECT 
        toInt64(toUnixTimestamp(toDateTime(anyLast(Timestamp)))) AS latest,
        Path AS metric,
        toFloat64(sum(Value)) AS value
    FROM default.graphite_data
    WHERE Path LIKE '{env}.%.{user_id}.uplink'
      AND Timestamp >= toDateTime('{start}')
      AND Timestamp < toDateTime('{start}') + INTERVAL 1 DAY
    GROUP BY Path",
            env = env,
            user_id = user_id,
            start = modified_at_str,
        );

        let client = self.client();
        let client = client.lock().await;

        let result = client.query(&query).fetch_all::<MetricValue<T>>().await;

        result.ok().and_then(|mut rows| rows.pop())
    }
}
