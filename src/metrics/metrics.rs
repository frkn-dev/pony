use std::fmt;
use std::io;

use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::TcpStream};

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

impl<T: ToString + std::fmt::Debug> Metric<T> {
    pub async fn send(&self, server: &str) -> Result<(), io::Error> {
        let metric_string = self.to_string();

        debug!("Send metric to carbon: {:?}", self);

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
}

#[derive(Debug, Clone)]
pub enum MetricType {
    F32(Metric<f32>),
    F64(Metric<f64>),
    I64(Metric<i64>),
    U64(Metric<u64>),
    U8(Metric<u8>),
}
