use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use crate::clickhouse::MetricValue;

#[derive(Debug, Clone)]
pub enum Stat {
    Conn(StatType),
    Inbound(StatType),
    Outbound(StatType),
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stat::Conn(StatType::Uplink) => write!(f, "uplink"),
            Stat::Conn(StatType::Downlink) => write!(f, "downlink"),
            Stat::Conn(StatType::Online) => write!(f, "online"),
            Stat::Inbound(StatType::Uplink) => write!(f, "uplink"),
            Stat::Inbound(StatType::Downlink) => write!(f, "downlink"),
            Stat::Inbound(StatType::Online) => write!(f, "Not implemented"),
            Stat::Outbound(StatType::Uplink) => write!(f, "uplink"),
            Stat::Outbound(StatType::Downlink) => write!(f, "downlink"),
            Stat::Outbound(StatType::Online) => write!(f, "Not implemented"),

            Stat::Conn(StatType::Unknown)
            | Stat::Inbound(StatType::Unknown)
            | Stat::Outbound(StatType::Unknown) => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatType {
    Uplink,
    Downlink,
    Online,
    Unknown,
}

impl StatType {
    pub fn from_path(path: &str) -> StatType {
        if let Some(last) = path.split('.').last() {
            match last {
                "uplink" => StatType::Uplink,
                "downlink" => StatType::Downlink,
                "online" => StatType::Online,
                _ => StatType::Unknown,
            }
        } else {
            StatType::Unknown
        }
    }
}

impl fmt::Display for StatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatType::Uplink => write!(f, "uplink"),
            StatType::Downlink => write!(f, "downlink"),
            StatType::Online => write!(f, "online"),
            StatType::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnStat {
    pub downlink: i64,
    pub uplink: i64,
    pub online: i64,
}

impl Default for ConnStat {
    fn default() -> Self {
        ConnStat {
            downlink: 0,
            uplink: 0,
            online: 0,
        }
    }
}

impl ConnStat {
    pub fn from_metrics<T: Into<i64> + Clone + Copy>(metrics: Vec<MetricValue<T>>) -> Self {
        let mut stat = ConnStat::default();
        for metric in metrics {
            let value = metric.value;
            match metric.stat_type() {
                StatType::Uplink => stat.uplink = value.into(),
                StatType::Downlink => stat.downlink = value.into(),
                StatType::Online => stat.online = value.into(),
                StatType::Unknown => {}
            }
        }
        stat
    }
}

impl fmt::Display for ConnStat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Uplink: {:.2} MB, Downlink: {:.2} MB, Online: {} ",
            self.uplink as f64 / 1_048_576.0,
            self.downlink as f64 / 1_048_576.0,
            self.online
        )
    }
}

pub struct InboundStat {
    pub downlink: i64,
    pub uplink: i64,
    pub conn_count: i64,
}
