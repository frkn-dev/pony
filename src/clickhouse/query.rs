use clickhouse::{Client, Row};
use log::debug;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Debug, Row, Serialize, Deserialize)]
pub struct MetricValue {
    pub latest: i64,
    pub metric: String,
    pub value: f64,
}

pub async fn fetch_heartbeat_value(
    client: Arc<Mutex<Client>>,
    env: &str,
    uuid: Uuid,
) -> Option<MetricValue> {
    let metric_value_req = format!(
        "SELECT 
            toInt64(toUnixTimestamp(toDateTime(anyLast(Timestamp)))) AS latest,
            Path AS metric,
            toFloat64(anyLast(Value)) AS value
        FROM default.graphite_data
        WHERE Path LIKE '{}.{}.heartbeat'
        GROUP BY Path",
        env, uuid
    );

    debug!("Running query - {}", metric_value_req);

    let client = client.lock().await;

    let result = client
        .query(&metric_value_req)
        .fetch_all::<MetricValue>()
        .await;

    match &result {
        Ok(rows) => debug!("Query result: {:?}", rows),
        Err(err) => debug!("Query error: {:?}", err),
    }

    result.ok().and_then(|hb| hb.into_iter().next())
}
