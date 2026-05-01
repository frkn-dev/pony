use dashmap::DashMap;
use std::collections::{BTreeMap, HashSet, VecDeque};

use super::{MetricEnvelope, MetricPoint};
use crate::memory::node::Node;
use crate::zmq::{publisher::Publisher, topic::Topic};

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
            .send_binary(&Topic::Metrics, bytes.as_slice())
            .await
        {
            tracing::error!("Batch publish failed: {}", e);
        }
    }
}

pub struct MetricStorage {
    pub inner: DashMap<uuid::Uuid, DashMap<u64, VecDeque<MetricPoint>>>,
    pub metadata: DashMap<u64, (String, BTreeMap<String, String>)>,
    pub tag_index: DashMap<String, DashMap<String, HashSet<u64>>>,

    pub max_points: usize,
    pub retention_seconds: i64,
}

impl MetricStorage {
    pub fn new(max_points: usize, retention_seconds: i64) -> Self {
        Self {
            inner: DashMap::new(),
            metadata: DashMap::new(),
            tag_index: DashMap::new(),
            max_points,
            retention_seconds,
        }
    }
    pub fn insert_envelope(&self, e: MetricEnvelope) {
        let key = Self::make_series_key(&e.name, &e.tags);

        self.metadata.entry(key).or_insert_with(|| {
            for (k, v) in &e.tags {
                self.tag_index
                    .entry(k.clone())
                    .or_default()
                    .entry(v.clone())
                    .or_default()
                    .insert(key);
            }
            (e.name.clone(), e.tags.clone())
        });

        let node_map = self.inner.entry(e.node_id).or_default();
        let mut entry = node_map.entry(key).or_default();

        entry.push_back(MetricPoint {
            timestamp: e.timestamp,
            value: e.value,
        });

        while entry.len() > self.max_points {
            entry.pop_front();
        }

        let retention_ms = self.retention_seconds * 1000;
        let min_ts = e.timestamp - retention_ms;

        while let Some(front) = entry.front() {
            if front.timestamp < min_ts {
                entry.pop_front();
            } else {
                break;
            }
        }
    }
    fn make_series_key(name: &str, tags: &BTreeMap<String, String>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        for (k, v) in tags {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        hasher.finish()
    }

    pub fn get_range(
        &self,
        node_id: &uuid::Uuid,
        series_hash: u64,
        from: i64,
        to: i64,
    ) -> Vec<MetricPoint> {
        self.inner
            .get(node_id)
            .and_then(|node_data| {
                node_data.get(&series_hash).map(|deque| {
                    deque
                        .iter()
                        .filter(|p| p.timestamp >= from && p.timestamp <= to)
                        .cloned()
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    pub fn find_series_by_tag(&self, tag_key: &str, tag_value: &str) -> HashSet<u64> {
        self.tag_index
            .get(tag_key)
            .and_then(|tag_map| tag_map.get(tag_value).map(|v| v.clone()))
            .unwrap_or_default()
    }

    pub fn get_aggregated_range(
        &self,
        tag_key: &str,
        tag_value: &str,
        metric_name: &str,
        from: i64,
        to: i64,
    ) -> BTreeMap<uuid::Uuid, Vec<MetricPoint>> {
        let mut result = BTreeMap::new();

        let hashes = self.find_series_by_tag(tag_key, tag_value);

        for node_ref in self.inner.iter() {
            let node_id = node_ref.key();
            let _node_data = node_ref.value();

            for hash in &hashes {
                if let Some(meta) = self.metadata.get(hash) {
                    if meta.0 == metric_name {
                        let points = self.get_range(node_id, *hash, from, to);
                        if !points.is_empty() {
                            result.insert(*node_id, points);
                        }
                    }
                }
            }
        }
        result
    }

    pub fn perform_gc(&self) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let retention_ms = self.retention_seconds * 1000;
        let min_ts = now_ms - retention_ms;

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

        let mut alive_series = std::collections::HashSet::new();
        for node_ref in self.inner.iter() {
            for key in node_ref.value().iter().map(|entry| *entry.key()) {
                alive_series.insert(key);
            }
        }

        self.metadata
            .retain(|series_key, _| alive_series.contains(series_key));

        self.tag_index.retain(|_tag_key, tag_map| {
            tag_map.retain(|_tag_value, set| {
                set.retain(|series_key| alive_series.contains(series_key));

                !set.is_empty()
            });

            !tag_map.is_empty()
        });
    }
}
