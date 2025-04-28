use std::collections::HashMap;

use super::metrics::{AsMetric, Metric, MetricType};
use crate::state::connection::Conn;
use crate::state::node::Node;
use crate::state::stats::{ConnStat, InboundStat};
use crate::utils::current_timestamp;

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
                value: self.conn_count,
                timestamp: timestamp,
            },
        ]
    }
}

impl AsMetric for ConnStat {
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

pub fn xray_stat_metrics(node: Node) -> Vec<MetricType> {
    let xray_stat_metrics: Vec<_> = node
        .inbounds
        .clone()
        .into_iter()
        .flat_map(|(tag, inbound)| {
            inbound
                .as_inbound_stat()
                .as_metric(&tag.to_string(), &node.env, &node.hostname)
        })
        .collect();

    xray_stat_metrics.into_iter().map(MetricType::I64).collect()
}

pub fn xray_conn_metrics(
    connections: HashMap<uuid::Uuid, Conn>,
    env: &str,
    hostname: &str,
) -> Vec<MetricType> {
    let conn_stat_metrics: Vec<_> = connections
        .clone()
        .into_iter()
        .map(|(user_id, user)| {
            user.as_conn_stat()
                .as_metric(&user_id.to_string(), env, hostname)
        })
        .flatten()
        .collect();

    conn_stat_metrics
        .iter()
        .map(|metric| MetricType::I64(metric.clone()))
        .collect()
}
