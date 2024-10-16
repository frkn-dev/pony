use clickhouse::{Client, Row};
use log::debug;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct MetricValue {
    pub latest: i64,
    pub metric: String,
    pub value: f64,
}

pub async fn fetch_metrics_value(
    client: &Client,
    env: &str,
    cluster: &str,
    metric_postfix: &str,
) -> Result<Vec<MetricValue>, Box<dyn Error>> {
    let metric_value_req = format!(
        "SELECT
                          toInt64(toUnixTimestamp(toDateTime(anyLast(Date)))) AS latest,
                          Path AS metric,
                          toFloat64(anyLast(Value)) AS value
                        FROM default.graphite_data
                        WHERE metric LIKE '{env}.{cluster}%.{metric_postfix}'
                        GROUP BY metric"
    );

    debug!("Running query - {metric_value_req}");

    let result = client
        .query(&metric_value_req)
        .fetch_all::<MetricValue>()
        .await?;

    for row in &result {
        debug!("{:?}", row);
    }

    Ok(result)
}
