use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::bandwidth::bandwidth_metrics;
use super::cpuusage::cpu_metrics;
use super::heartbeat::heartbeat_metrics;
use super::loadavg::loadavg_metrics;
use super::memory::mem_metrics;
use super::xray::{xray_stat_metrics, xray_user_metrics};
use crate::state::state::State;

pub trait AsMetric {
    type Output;

    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<Self::Output>>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Metric<T> {
    pub path: String,
    pub value: T,
    pub timestamp: u64,
}

impl<T> Metric<T> {
    pub fn new(path: String, value: T, timestamp: u64) -> Self {
        Metric {
            path,
            value,
            timestamp,
        }
    }
}

impl<T: Default> Default for Metric<T> {
    fn default() -> Self {
        Metric {
            value: T::default(),
            path: String::from(""),
            timestamp: 0,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Metric<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Metric {{ path: {}, value: {},  timestamp: {} }}",
            self.path, self.value, self.timestamp
        )
    }
}

impl<T: ToString> Metric<T> {
    pub fn to_string(&self) -> String {
        format!(
            "{} {} {}\n",
            self.path,
            self.value.to_string(),
            self.timestamp
        )
    }
}

#[derive(Debug, Clone)]
pub enum MetricType {
    F32(Metric<f32>),
    F64(Metric<f64>),
    I64(Metric<i64>),
    U64(Metric<u64>),
    U8(Metric<u8>),
}

pub async fn collect_metrics<T>(
    state: Arc<Mutex<State>>,
    env: &str,
    hostname: &str,
    interface: &str,
    node_id: Uuid,
) -> Vec<MetricType> {
    let mut metrics: Vec<MetricType> = Vec::new();
    let bandwidth: Vec<MetricType> = bandwidth_metrics(env, hostname, interface).await;
    let cpuusage: Vec<MetricType> = cpu_metrics(env, hostname).await;
    let loadavg: Vec<MetricType> = loadavg_metrics(env, hostname).await;
    let memory: Vec<MetricType> = mem_metrics(env, hostname).await;
    let xray: Vec<MetricType> = xray_stat_metrics(state.clone(), env, hostname, node_id).await;
    let users: Vec<MetricType> = xray_user_metrics(state, env, hostname).await;
    let heartbeat: Vec<MetricType> = heartbeat_metrics(env, hostname);

    metrics.extend(bandwidth);
    metrics.extend(cpuusage);
    metrics.extend(loadavg);
    metrics.extend(memory);
    metrics.extend(xray);
    metrics.extend(users);
    metrics.extend(heartbeat);

    metrics
}
