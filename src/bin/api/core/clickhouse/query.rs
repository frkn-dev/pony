use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use uuid::Uuid;

use super::ChContext;
use pony::metrics::metrics::Metric;

#[async_trait]
pub trait Queries {
    async fn fetch_node_heartbeat(
        &self,
        env: &str,
        uuid: &Uuid,
        hostname: &str,
    ) -> Option<Metric<u8>>;

    async fn fetch_conn_stats(
        &self,
        conn_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> Option<Vec<Metric<i64>>>;

    async fn fetch_node_cpu_load_avg1(&self, env: &str, hostname: &str) -> Option<Metric<f64>>;
    async fn fetch_node_cpu_load_avg5(&self, env: &str, hostname: &str) -> Option<Metric<f64>>;
    async fn fetch_node_bandwidth(
        &self,
        env: &str,
        hostname: &str,
        interface: &str,
    ) -> Option<Metric<f64>>;
    async fn fetch_node_cpuusage_max(&self, env: &str, hostname: &str) -> Option<Metric<f64>>;
    async fn fetch_node_memory_usage_ratio(&self, env: &str, hostname: &str)
        -> Option<Metric<f64>>;
}

#[async_trait]
impl Queries for ChContext {
    async fn fetch_node_heartbeat(
        &self,
        env: &str,
        uuid: &Uuid,
        hostname: &str,
    ) -> Option<Metric<u8>> {
        let query = format!(
            r#"
SELECT Path AS metric,
       toUInt8(argMax(Value, Timestamp)) AS value,
       toInt64(toUnixTimestamp(toDateTime(argMax(Timestamp, Timestamp)))) AS timestamp
FROM default.graphite_data
WHERE Path = '{}.{}.{}.heartbeat'
GROUP BY Path"#,
            env, hostname, uuid
        );
        self.execute_metric_query(&query).await
    }

    async fn fetch_conn_stats(
        &self,
        conn_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> Option<Vec<Metric<i64>>> {
        let start_str = start.format("%Y-%m-%d %H:%M:%S").to_string();

        let query = format!(
            r#"
SELECT  metric,
    toInt64(sum(metric_value)) AS value,
    anyLast(timestamp) AS timestamp 
FROM (
  SELECT
    toInt64(toUnixTimestamp(toDateTime(anyLast(Timestamp)))) AS timestamp,
    extract(Path, '[^.]+$') AS metric,
    anyLast(Value) AS metric_value
  FROM default.graphite_data
  WHERE Path LIKE '%.%.{conn_id}.conn_stat.%'
     AND Timestamp >= toDateTime('{start_str}')
     AND Timestamp < toDateTime('{start_str}') + INTERVAL 1 DAY
  GROUP BY Path
 )
GROUP BY metric"#,
        );
        let client = self.client();

        let result = client.query(&query).fetch_all::<Metric<i64>>().await;

        match &result {
            Ok(rows) => {
                log::debug!("Fetched {} connection stats rows", rows.len());
                Some(rows.clone())
            }
            Err(e) => {
                log::error!("Failed to fetch connection stats: {:?}", e);
                None
            }
        }
    }

    async fn fetch_node_cpu_load_avg1(&self, env: &str, hostname: &str) -> Option<Metric<f64>> {
        let query = format!(
            r#"
SELECT
    any(Path) AS metric,
    avg(toFloat64(Value)) AS value,
    toInt64(toUnixTimestamp(now())) AS timestamp
FROM default.graphite_data
WHERE Path = '{}.{}.loadavg.5m'
AND Timestamp >= now() - INTERVAL 5 MINUTE"#,
            env, hostname
        );
        self.execute_metric_query(&query).await
    }

    async fn fetch_node_cpu_load_avg5(&self, env: &str, hostname: &str) -> Option<Metric<f64>> {
        let query = format!(
            r#"
SELECT
    any(Path) AS metric,
    avg(toFloat64(Value)) AS value,
    toInt64(toUnixTimestamp(now())) AS timestamp
FROM default.graphite_data
WHERE Path = '{}.{}.loadavg.5m'
AND Timestamp >= now() - INTERVAL 5 MINUTE"#,
            env, hostname
        );
        self.execute_metric_query(&query).await
    }

    async fn fetch_node_bandwidth(
        &self,
        env: &str,
        hostname: &str,
        interface: &str,
    ) -> Option<Metric<f64>> {
        let query = format!(
            r#"
SELECT
    any(Path) AS metric,
    avg(toFloat64(Value)) AS value,
    toInt64(toUnixTimestamp(now())) AS timestamp
FROM default.graphite_data
WHERE Path = '{}.{}.network.{}.tx_bps'
AND Timestamp >= now() - INTERVAL 5 MINUTE"#,
            env, hostname, interface
        );

        self.execute_metric_query(&query).await
    }

    async fn fetch_node_cpuusage_max(&self, env: &str, hostname: &str) -> Option<Metric<f64>> {
        let query = format!(
            r#"
SELECT
    Path AS metric,
    toFloat64(argMax(Value, Timestamp)) AS value,
    toInt64(toUnixTimestamp(toDateTime(argMax(Timestamp, Timestamp)))) AS timestamp
FROM default.graphite_data
WHERE Path LIKE '{}.{}.cpu.%.percentage'
GROUP BY Path
ORDER BY value DESC
LIMIT 1
"#,
            env, hostname,
        );
        self.execute_metric_query(&query).await
    }

    async fn fetch_node_memory_usage_ratio(
        &self,
        env: &str,
        hostname: &str,
    ) -> Option<Metric<f64>> {
        let query = format!(
            r#"
        SELECT
            used.value / total.value AS value,
            toInt64(toUnixTimestamp(now())) AS timestamp,
            'mem.used/mem.total' AS metric
        FROM
            (
                SELECT argMax(Value, Timestamp) AS value
                FROM default.graphite_data
                WHERE Path = '{0}.{1}.mem.used'
            ) AS used,
            (
                SELECT argMax(Value, Timestamp) AS value
                FROM default.graphite_data
                WHERE Path = '{0}.{1}.mem.total'
            ) AS total
        "#,
            env, hostname
        );

        let client = self.client();
        let row_result = client.query(&query).fetch_one::<(f64, i64, String)>().await;

        match row_result {
            Ok((value, timestamp, metric)) => Some(Metric {
                value,
                timestamp,
                metric,
            }),
            Err(err) => {
                log::error!("Failed to fetch memory usage ratio: {:?}", err);
                None
            }
        }
    }
}
