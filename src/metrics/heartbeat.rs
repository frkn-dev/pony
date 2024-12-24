use super::metrics::{Metric, MetricType};
use crate::utils::current_timestamp;

pub fn heartbeat_metrics(env: &str, hostname: &str) -> Vec<MetricType> {
    let timestamp = current_timestamp();

    //dev.localhost.heartbeat
    let path = format!("{env}.{hostname}.heartbeat");
    let metric = Metric::new(path, 1, timestamp);

    vec![MetricType::U8(metric)]
}
