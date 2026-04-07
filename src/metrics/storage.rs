use dashmap::DashMap;
use std::collections::{BTreeMap, VecDeque};

use super::MetricEnvelope;
use super::MetricPoint;
use crate::memory::node::Node;
use crate::zmq::publisher::Publisher;

pub trait HasMetrics {
    fn metrics(&self) -> &MetricBuffer;
    fn node_settings(&self) -> &Node;
}

pub trait MetricSink {
    fn write(&self, node_id: &uuid::Uuid, metric: &str, value: f64, tags: BTreeMap<String, String>);
}

impl MetricSink for MetricBuffer {
    fn write(
        &self,
        node_id: &uuid::Uuid,
        metric: &str,
        value: f64,
        tags: BTreeMap<String, String>,
    ) {
        let mut b = self.batch.lock();
        b.push(MetricEnvelope {
            node_id: *node_id,
            name: metric.to_string(),
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
            tags,
        });
    }
}

pub struct MetricBuffer {
    pub batch: parking_lot::Mutex<Vec<MetricEnvelope>>,
    pub publisher: Publisher,
}

impl MetricBuffer {
    pub fn push(
        &self,
        node_id: uuid::Uuid,
        name: &str,
        value: f64,
        tags: BTreeMap<String, String>,
    ) {
        let mut b = self.batch.lock();
        b.push(MetricEnvelope {
            node_id,
            name: name.to_string(),
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
            tags,
        });
    }

    pub async fn flush_to_zmq(&self) {
        let metrics = {
            let mut batch = self.batch.lock();
            if batch.is_empty() {
                return;
            }
            std::mem::take(&mut *batch)
        };

        let bytes = rkyv::to_bytes::<_, 65536>(&metrics).expect("Failed to serialize batch");

        if let Err(e) = &self
            .publisher
            .send_binary("metrics", bytes.as_slice())
            .await
        {
            log::error!("Batch publish failed: {}", e);
        }
    }
}

pub struct MetricStorage {
    pub inner: DashMap<uuid::Uuid, DashMap<String, VecDeque<MetricPoint>>>,
    pub metadata: DashMap<String, BTreeMap<String, String>>,

    pub max_points: usize,
    pub retention_seconds: i64,
}

impl MetricStorage {
    pub fn new(max_points: usize, retention_seconds: i64) -> Self {
        Self {
            inner: DashMap::new(),
            metadata: DashMap::new(),
            max_points,
            retention_seconds,
        }
    }
    fn make_series_key(name: &str, tags: &BTreeMap<String, String>) -> String {
        if tags.is_empty() {
            return name.to_string();
        }
        let tags_part: Vec<String> = tags.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        format!("{}:{{{}}}", name, tags_part.join(","))
    }

    pub fn get_range(
        &self,
        node_id: &uuid::Uuid,
        series_key: &str,
        from: i64,
        to: i64,
    ) -> Vec<MetricPoint> {
        self.inner
            .get(node_id)
            .and_then(|node_data| {
                node_data.get(series_key).map(|deque| {
                    deque
                        .iter()
                        .filter(|p| p.timestamp >= from && p.timestamp <= to)
                        .cloned()
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    pub fn insert_envelope(&self, env: MetricEnvelope) {
        let series_key = Self::make_series_key(&env.name, &env.tags);

        if !self.metadata.contains_key(&series_key) {
            self.metadata.insert(series_key.clone(), env.tags);
        }

        let node_map = self.inner.entry(env.node_id).or_default();
        let mut entry = node_map.entry(series_key).or_default();

        // env.timestamp уже в миллисекундах (из MetricBuffer)
        entry.push_back(MetricPoint {
            timestamp: env.timestamp,
            value: env.value,
        });

        // 1. Очистка по количеству точек
        while entry.len() > self.max_points {
            entry.pop_front();
        }

        // 2. Очистка по времени (Retention)
        // Переводим секунды конфига в мс для сравнения с env.timestamp
        let retention_ms = self.retention_seconds * 1000;
        let min_ts = env.timestamp - retention_ms;

        while let Some(front) = entry.front() {
            if front.timestamp < min_ts {
                entry.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn perform_gc(&self) {
        // Используем встроенный метод для мс, чтобы не множить на 1000 вручную
        let now_ms = chrono::Utc::now().timestamp_millis();
        let retention_ms = self.retention_seconds * 1000;
        let min_ts = now_ms - retention_ms;

        log::debug!("Starting MetricStorage GC. Min timestamp: {}", min_ts);

        self.inner.retain(|_node_id, node_map| {
            node_map.retain(|_series_key, deque| {
                while let Some(front) = deque.front() {
                    if front.timestamp < min_ts {
                        deque.pop_front();
                    } else {
                        break;
                    }
                }
                !deque.is_empty()
            });
            !node_map.is_empty()
        });

        self.metadata.retain(|series_key, _| {
            self.inner
                .iter()
                .any(|node_ref| node_ref.value().contains_key(series_key))
        });
    }
}
