use sysinfo::{LoadAvg, System};

use crate::{
    metrics::metrics::{AsMetric, Metric},
    utils::current_timestamp,
};

use super::metrics::MetricType;

struct LoadAvgWrapper {
    load_avg: LoadAvg,
}

impl AsMetric for LoadAvgWrapper {
    type Output = f64;

    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<f64>> {
        let timestamp = current_timestamp();
        vec![
            Metric {
                //dev.localhost.loadavg.1m
                path: format!("{env}.{hostname}.{name}.1m"),
                value: self.load_avg.one,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.loadavg.5m
                path: format!("{env}.{hostname}.{name}.5m"),
                value: self.load_avg.five,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.loadavg.15m
                path: format!("{env}.{hostname}.{name}.15m"),
                value: self.load_avg.fifteen,
                timestamp: timestamp,
            },
        ]
    }
}

pub fn loadavg_metrics(env: &str, hostname: &str) -> Vec<MetricType> {
    let load_avg = System::load_average();
    let wrapper = LoadAvgWrapper { load_avg };

    let load_metrics = wrapper.as_metric("loadavg", env, hostname);

    load_metrics
        .iter()
        .map(|metric| MetricType::F64(metric.clone()))
        .collect()
}
