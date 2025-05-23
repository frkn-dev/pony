use super::metrics::{AsMetric, Metric, MetricType};
use crate::state::ConnBaseOp;
use crate::state::Connections;
use crate::state::Node;
use crate::state::{ConnStat, InboundStat};
use crate::utils::current_timestamp;

impl AsMetric for InboundStat {
    type Output = i64;
    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<i64>> {
        let timestamp = current_timestamp();

        vec![
            Metric {
                //dev.localhost.vmess.inbound_stat.uplink
                path: format!("{env}.{hostname}.{name}.inbound_stat.uplink"),
                value: self.uplink,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.vmess.inbound_stat.downlink
                path: format!("{env}.{hostname}.{name}.inbound_stat.downlink"),
                value: self.downlink,
                timestamp: timestamp,
            },
            Metric {
                // dev.localhost.vmess.inbound_stat.user_count
                path: format!("{env}.{hostname}.{name}.inbound_stat.user_count"),
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
                path: format!("{env}.{hostname}.{name}.conn_stat.uplink"),
                value: self.uplink,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.user_id.downlink
                path: format!("{env}.{hostname}.{name}.conn_stat.downlink"),
                value: self.downlink,
                timestamp: timestamp,
            },
            Metric {
                // dev.localhost.user_id.online
                path: format!("{env}.{hostname}.{name}.conn_stat.online"),
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

pub fn xray_conn_metrics<C>(
    connections: Connections<C>,
    env: &str,
    hostname: &str,
) -> Vec<MetricType>
where
    C: ConnBaseOp + Send + Sync + Clone + 'static,
{
    let conn_stat_metrics: Vec<_> = connections
        .clone()
        .iter()
        .map(|(conn_id, conn)| {
            conn.as_conn_stat()
                .as_metric(&conn_id.to_string(), env, hostname)
        })
        .flatten()
        .collect();

    conn_stat_metrics
        .iter()
        .map(|metric| MetricType::I64(metric.clone()))
        .collect()
}
