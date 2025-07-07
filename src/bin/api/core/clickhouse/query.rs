use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use uuid::Uuid;

use super::ChContext;
use pony::metrics::metrics::Metric;

#[async_trait]
pub trait Queries {
    async fn fetch_node_heartbeat<T>(
        &self,
        env: &str,
        uuid: &Uuid,
        hostname: &str,
    ) -> Option<Metric<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static;

    async fn fetch_conn_stats<T>(
        &self,
        conn_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> Option<Vec<Metric<T>>>
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
    ) -> Option<Metric<T>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        let query = format!(
            r#"SELECT Path AS metric,
          toFloat64(argMax(Value, Timestamp)) AS value,
          toInt64(toUnixTimestamp(toDateTime(argMax(Timestamp, Timestamp)))) AS timestamp     
FROM default.graphite_data
WHERE Path = '{}.{}.{}.heartbeat'
GROUP BY Path"#,
            env, hostname, uuid
        );

        println!("CH Query\n{}", query);

        let client = self.client();

        let result = client.query(&query).fetch_all::<Metric<T>>().await;

        match result {
            Ok(ref rows) => {
                log::debug!("Fetched {} rows from CH", rows.len());
            }
            Err(ref e) => {
                log::error!("Failed to fetch heartbeat from CH: {:?}", e);
            }
        }

        result.ok().and_then(|mut rows| rows.pop())
    }

    async fn fetch_conn_stats<T>(
        &self,
        conn_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> Option<Vec<Metric<T>>>
    where
        T: DeserializeOwned + Debug + Send + Clone + Sync + 'static,
    {
        let start_str = start.format("%Y-%m-%d %H:%M:%S").to_string();
        let query = format!(
            r#"SELECT  metric, 
           toInt64(sum(metric_value)) AS value,
           anyLast(timestamp) AS timestamp 
FROM (
  SELECT
    toInt64(toUnixTimestamp(toDateTime(anyLast(Timestamp)))) AS timestamp,
    extract(Path, '[^.]+$') AS metric,
    anyLast(Value) AS metric_value
  FROM default.graphite_data
  WHERE Path LIKE '%.%.{conn_id}.conn_stat.%'
     AND Timestamp >= toDateTime('{start}')
     AND Timestamp < toDateTime('{start}') + INTERVAL 1 DAY
  GROUP BY Path
 )
GROUP BY metric"#,
            conn_id = conn_id,
            start = start_str,
        );

        println!("CH Query\n{}", query);

        let client = self.client();

        let result = client.query(&query).fetch_all::<Metric<T>>().await;

        result.ok()
    }
}
