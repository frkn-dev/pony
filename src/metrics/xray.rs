use std::collections::HashMap;

use log::debug;
use uuid::Uuid;

use crate::state::stats::{InboundStat, UserStat};
use crate::state::user::User;
use crate::utils::current_timestamp;
use crate::Node;
use crate::{AsMetric, Metric, MetricType};

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

pub fn xray_user_metrics(users: HashMap<Uuid, User>, env: &str, hostname: &str) -> Vec<MetricType> {
    let user_stat_metrics: Vec<_> = users
        .clone()
        .into_iter()
        .map(|(user_id, user)| {
            debug!("user {:?}", user_id);
            user.as_user_stat()
                .as_metric(&user_id.to_string(), env, hostname)
        })
        .flatten()
        .collect();

    debug!("user_stat_metrics {:?}", user_stat_metrics);

    user_stat_metrics
        .iter()
        .map(|metric| MetricType::I64(metric.clone()))
        .collect()
}
