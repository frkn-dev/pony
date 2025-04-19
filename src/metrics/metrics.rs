use std::fmt;
use std::io;
use std::sync::Arc;

use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::TcpStream};

use super::bandwidth::bandwidth_metrics;
use super::cpuusage::cpu_metrics;
use super::heartbeat::heartbeat_metrics;
use super::loadavg::loadavg_metrics;
use super::memory::mem_metrics;
use super::xray::{xray_stat_metrics, xray_user_metrics};
use crate::state::state::NodeStorage;
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

pub async fn collect_metrics<T>(state: Arc<Mutex<State<T>>>) -> Vec<MetricType>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let mut metrics: Vec<MetricType> = Vec::new();
    let state = state.lock().await;
    let users = state.users.clone();

    let node = state.nodes.get_node();

    if let Some(node) = node {
        let bandwidth: Vec<MetricType> = bandwidth_metrics(&node.env, &node.hostname, &node.iface);
        let cpuusage: Vec<MetricType> = cpu_metrics(&node.env, &node.hostname);
        let loadavg: Vec<MetricType> = loadavg_metrics(&node.env, &node.hostname);
        let memory: Vec<MetricType> = mem_metrics(&node.env, &node.hostname);
        let heartbeat: Vec<MetricType> = heartbeat_metrics(&node.env, node.uuid);
        let xray_stat: Vec<MetricType> = xray_stat_metrics(node.clone());
        let users_stat: Vec<MetricType> = xray_user_metrics(users, &node.env, &node.hostname);

        metrics.extend(bandwidth);
        metrics.extend(cpuusage);
        metrics.extend(loadavg);
        metrics.extend(memory);
        metrics.extend(xray_stat);
        metrics.extend(users_stat);
        metrics.extend(heartbeat);
    }

    debug!("Total metrics collected: {}", metrics.len());

    metrics
}

pub async fn send_to_carbon<T: ToString + std::fmt::Debug>(
    metric: &Metric<T>,
    server: &str,
) -> Result<(), io::Error> {
    let metric_string = metric.to_string();

    debug!("Send metric to carbon: {:?}", metric);

    match TcpStream::connect(server).await {
        Ok(mut stream) => {
            if let Err(e) = stream.write_all(metric_string.as_bytes()).await {
                warn!("Failed to send metric: {}", e);
                return Err(e);
            }

            if let Err(e) = stream.flush().await {
                warn!("Failed to flush stream: {}", e);
                return Err(e);
            }

            Ok(())
        }
        Err(e) => {
            error!("Failed to connect to Carbon server: {}", e);
            Err(e)
        }
    }
}
