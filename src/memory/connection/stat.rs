use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use super::super::stat::Kind;
use crate::metrics::metrics::Metric;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Stat {
    pub downlink: i64,
    pub uplink: i64,
    pub online: i64,
}

impl Default for Stat {
    fn default() -> Self {
        Stat {
            downlink: 0,
            uplink: 0,
            online: 0,
        }
    }
}

impl Stat {
    pub fn from_metrics<T: Into<i64> + Clone + Copy>(metrics: Vec<Metric<T>>) -> Self {
        let mut stat = Stat::default();
        for metric in metrics {
            let value = metric.value;
            match metric.stat_type() {
                Kind::Uplink => stat.uplink = value.into(),
                Kind::Downlink => stat.downlink = value.into(),
                Kind::Online => stat.online = value.into(),
                Kind::Unknown => {}
            }
        }
        stat
    }
}

impl fmt::Display for Stat {
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
