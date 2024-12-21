use super::metrics::AsMetric;
use super::metrics::Metric;
use super::metrics::MetricType;
use crate::current_timestamp;
use crate::state::State;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct InboundStat {
    pub uplink: i64,
    pub downlink: i64,
    pub user_count: i64,
}

impl AsMetric for InboundStat {
    type Output = i64;
    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<i64>> {
        let timestamp = current_timestamp();

        vec![
            Metric {
                //dev.localhost.vmess.uplink
                path: format!("{env}.{hostname}.{name}.uplink"),
                value: self.uplink,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.vmess.downlink
                path: format!("{env}.{hostname}.{name}.downlink"),
                value: self.downlink,
                timestamp: timestamp,
            },
            Metric {
                // dev.localhost.vmess.user_count
                path: format!("{env}.{hostname}.{name}.user_count"),
                value: self.user_count,
                timestamp: timestamp,
            },
        ]
    }
}

pub async fn xray_stat_metrics(
    state: Arc<Mutex<State>>,
    env: &str,
    hostname: &str,
) -> Vec<MetricType> {
    let state = state.lock().await;
    let node = state.node.clone();

    let xray_stat_metrics: Vec<_> = node
        .inbounds
        .into_iter()
        .map(|(tag, inbound)| {
            inbound
                .as_inbound_stat()
                .as_metric(&tag.to_string(), env, hostname)
        })
        .flatten()
        .collect();

    xray_stat_metrics
        .iter()
        .map(|metric| MetricType::I64(metric.clone()))
        .collect()
}
