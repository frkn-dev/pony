use clickhouse::Row;
use std::fmt;
use std::io;

use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::state::stat::Kind as StatKind;

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
        write!(
            f,
            "Metric {{ path: {}, value: {},  timestamp: {} }}",
            self.metric, self.value, self.timestamp
        )
    }
}

impl<T: std::fmt::Display + std::fmt::Debug> Metric<T> {
    pub async fn send(&self, server: &str) -> Result<(), io::Error> {
        let metric_string = self.to_string();

        log::debug!("Send metric to carbon: {:?}", self);

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
