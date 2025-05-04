use super::metrics::{Metric, MetricType};
use crate::utils::current_timestamp;

pub fn heartbeat_metrics(env: &str, uuid: &uuid::Uuid, hostname: &str) -> Vec<MetricType> {
    let timestamp = current_timestamp();

    //dev.<uuid>.heartbeat
    let path = format!("{env}.{hostname}.{uuid}.heartbeat");
    let metric = Metric::new(path, 1, timestamp);

    vec![MetricType::U8(metric)]
}
