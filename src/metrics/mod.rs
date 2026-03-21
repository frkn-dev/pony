use crate::{PonyError, Result as PonyResult};

pub mod bandwidth;
pub mod cpuusage;
pub mod heartbeat;
pub mod loadavg;
pub mod memory;
pub mod xray;

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::memory::stat::Kind as StatKind;

pub trait AsMetric {
    type Output;
    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<Self::Output>>;
}

#[derive(Serialize, Deserialize, Row, Clone, Debug)]
pub struct Metric<T> {
    pub metric: String,
    pub value: T,
    pub timestamp: i64,
}

impl<T> Metric<T> {
    pub fn new(metric: String, value: T, timestamp: i64) -> Self {
        Metric {
            metric,
            value,
            timestamp,
        }
    }
    pub fn stat_type(&self) -> StatKind {
        StatKind::from_path(&self.metric)
    }
}

impl<T: Default> Default for Metric<T> {
    fn default() -> Self {
        Metric {
            value: T::default(),
            metric: String::from(""),
            timestamp: 0,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Metric<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.metric, self.value, self.timestamp)
    }
}

impl<T: std::fmt::Display + std::fmt::Debug> Metric<T> {
    pub async fn send(&self, server: &str) -> Result<(), io::Error> {
        let metric_string = format!("{self}\n");

        log::debug!("Sending carbon metric string: {:?}", metric_string);

        match TcpStream::connect(server).await {
            Ok(mut stream) => {
                if let Err(e) = stream.write_all(metric_string.as_bytes()).await {
                    log::warn!("Failed to send metric: {}", e);
                    return Err(e);
                }

                if let Err(e) = stream.flush().await {
                    log::warn!("Failed to flush stream: {}", e);
                    return Err(e);
                }

                if let Err(e) = stream.shutdown().await {
                    log::warn!("Failed to shutdown stream: {}", e);
                    return Err(e);
                }

                Ok(())
            }
            Err(e) => {
                log::error!("Failed to connect to Carbon server: {}", e);
                Err(e)
            }
        }
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

#[async_trait::async_trait]
pub trait Metrics<T> {
    async fn collect_metrics<M>(&self) -> Vec<MetricType>;
    async fn collect_hb_metrics<M>(&self) -> MetricType;

    async fn send_metrics(&self, carbon_address: &str) -> PonyResult<()> {
        let metrics = self.collect_metrics::<T>().await;

        for metric in metrics {
            match metric {
                MetricType::F32(m) => m.send(carbon_address).await?,
                MetricType::F64(m) => m.send(carbon_address).await?,
                MetricType::I64(m) => m.send(carbon_address).await?,
                MetricType::U64(m) => m.send(carbon_address).await?,
                MetricType::U8(m) => m.send(carbon_address).await?,
            }
        }
        Ok(())
    }
    async fn send_hb_metric(&self, carbon_address: &str) -> PonyResult<()> {
        let metric = self.collect_hb_metrics::<f64>().await;

        match metric {
            MetricType::F64(m) => {
                m.send(carbon_address).await?;
                Ok(())
            }
            _ => Err(PonyError::Custom("Doesn't support metric type".to_string())),
        }
    }
}
