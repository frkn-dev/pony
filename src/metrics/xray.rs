use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::metrics::{AsMetric, Metric, MetricType};
use crate::utils::current_timestamp;

use crate::state::{
    state::State,
    stats::{InboundStat, UserStat},
};

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

impl AsMetric for UserStat {
    type Output = i64;
    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<i64>> {
        let timestamp = current_timestamp();

        vec![
            Metric {
                //dev.localhost.user_id.uplink
                path: format!("{env}.{hostname}.{name}.uplink"),
                value: self.uplink,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.user_id.downlink
                path: format!("{env}.{hostname}.{name}.downlink"),
                value: self.downlink,
                timestamp: timestamp,
            },
            Metric {
                // dev.localhost.user_id.online
                path: format!("{env}.{hostname}.{name}.online"),
                value: self.online,
                timestamp: timestamp,
            },
        ]
    }
}

pub async fn xray_stat_metrics(
    state: Arc<Mutex<State>>,
    env: &str,
    hostname: &str,
    node_id: Uuid,
) -> Vec<MetricType> {
    let state = state.lock().await;
    if let Some(node) = state.get_node(env.to_string(), node_id) {
        let xray_stat_metrics: Vec<_> = node
            .inbounds
            .clone()
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
    } else {
        vec![]
    }
}

pub async fn xray_user_metrics(
    state: Arc<Mutex<State>>,
    env: &str,
    hostname: &str,
) -> Vec<MetricType> {
    let state = state.lock().await;
    let users = state.users.clone();

    let user_stat_metrics: Vec<_> = users
        .into_iter()
        .map(|(user_id, user)| {
            user.as_user_stat()
                .as_metric(&user_id.to_string(), env, hostname)
        })
        .flatten()
        .collect();

    user_stat_metrics
        .iter()
        .map(|metric| MetricType::I64(metric.clone()))
        .collect()
}
