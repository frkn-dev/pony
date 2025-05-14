use super::metrics::{Metric, MetricType};
use crate::utils::current_timestamp;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

static BEAT_INDEX: OnceLock<AtomicUsize> = OnceLock::new();

pub fn heartbeat_metrics(env: &str, uuid: &uuid::Uuid, hostname: &str) -> MetricType {
    let timestamp = current_timestamp();
    let path = format!("{env}.{hostname}.{uuid}.heartbeat");

    let pattern: &[f64] = &[
        1.0, 1.0, 2.5, 0.4, 0.6, 1.3, 1.0, 1.0, 1.1, 1.0, 1.0, 0.8, 1.0, 1.9, 0.3, 0.5, 1.0,
    ];

    let value = {
        let index = BEAT_INDEX
            .get_or_init(|| AtomicUsize::new(0))
            .fetch_add(1, Ordering::Relaxed)
            % pattern.len();
        pattern[index]
    };

    let metric = Metric::new(path, value, timestamp);
    MetricType::F64(metric)
}
