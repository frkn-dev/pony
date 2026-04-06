use dashmap::DashMap;
use std::collections::VecDeque;

use super::MetricEnvelope;
use super::MetricPoint;
use crate::zmq::publisher::Publisher;

pub trait MetricSink {
    fn write(&self, node_id: &uuid::Uuid, metric: &str, value: f64, timestamp: i64);
}

pub trait HasMetrics {
    fn metrics(&self) -> &MetricStorage;
    fn node_id(&self) -> &uuid::Uuid;
}

impl MetricSink for MetricStorage {
    fn write(&self, node_id: &uuid::Uuid, metric: &str, value: f64, timestamp: i64) {
        self.insert(metric, value, timestamp);

        if let Some(publ) = &self.publisher {
            let envelope = MetricEnvelope {
                node_id: *node_id,
                name: metric.to_string(),
                value,
                timestamp,
            };

            let bytes = rkyv::to_bytes::<_, 1024>(&envelope).expect("Failed to serialize metric");

            let p = publ.clone();
            let topic = "metrics".to_string();
            let payload = bytes.into_vec();

            tokio::spawn(async move {
                if let Err(e) = p.send_binary(&topic, &payload).await {
                    log::error!("Failed to pub metric to ZMQ: {}", e);
                }
            });
        }
    }
}

pub struct MetricStorage {
    inner: DashMap<String, VecDeque<MetricPoint>>,
    max_points: usize,
    retention_seconds: i64,
    pub publisher: Option<Publisher>,
}

impl MetricStorage {
    pub fn new(max_points: usize, retention_seconds: i64, publisher: Option<Publisher>) -> Self {
        Self {
            inner: DashMap::new(),
            max_points,
            retention_seconds,
            publisher,
        }
    }

    pub fn insert(&self, metric: &str, value: f64, timestamp: i64) {
        let mut entry = self.inner.entry(metric.to_string()).or_default();

        entry.push_back(MetricPoint { timestamp, value });

        while entry.len() > self.max_points {
            entry.pop_front();
        }

        let min_ts = timestamp - self.retention_seconds;

        while let Some(front) = entry.front() {
            if front.timestamp < min_ts {
                entry.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn get_last(&self, metric: &str, n: usize) -> Option<Vec<MetricPoint>> {
        self.inner.get(metric).map(|deque| {
            deque
                .iter()
                .rev()
                .take(n)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        })
    }

    pub fn get_range(&self, metric: &str, from: i64, to: i64) -> Option<Vec<MetricPoint>> {
        self.inner.get(metric).map(|deque| {
            deque
                .iter()
                .filter(|p| p.timestamp >= from && p.timestamp <= to)
                .cloned()
                .collect()
        })
    }

    pub fn last(&self, n: usize) -> Vec<(String, f64)> {
        let mut metrics_with_last_ts: Vec<(String, i64, f64)> = self
            .inner
            .iter()
            .filter_map(|entry| {
                entry
                    .value()
                    .back()
                    .map(|point| (entry.key().clone(), point.timestamp, point.value))
            })
            .collect();

        metrics_with_last_ts.sort_by(|a, b| b.1.cmp(&a.1));

        metrics_with_last_ts
            .into_iter()
            .take(n)
            .map(|(name, _, value)| (name, value))
            .collect()
    }
}
