use crate::metrics::metrics::MetricType;
use crate::Result;

pub mod bandwidth;
pub mod cpuusage;
pub mod heartbeat;
pub mod loadavg;
pub mod memory;
pub mod metrics;
pub mod xray;

#[async_trait::async_trait]
pub trait Metrics<T> {
    async fn collect_metrics<M>(&self) -> Vec<MetricType>;

    async fn send_metrics(&self, carbon_address: String) -> Result<()> {
        let metrics = self.collect_metrics::<T>().await;

        for metric in metrics {
            match metric {
                MetricType::F32(m) => m.send(&carbon_address).await?,
                MetricType::F64(m) => m.send(&carbon_address).await?,
                MetricType::I64(m) => m.send(&carbon_address).await?,
                MetricType::U64(m) => m.send(&carbon_address).await?,
                MetricType::U8(m) => m.send(&carbon_address).await?,
            }
        }
        Ok(())
    }
}
