use std::fmt;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use crate::{
    metrics::metrics::{AsMetric, Metric, MetricType},
    utils::{current_timestamp, round_to_two_decimal_places},
};

struct CpuUsage<'a> {
    name: &'a str,
    usage: f32,
}

impl Default for CpuUsage<'_> {
    fn default() -> Self {
        CpuUsage {
            name: "0",
            usage: 0.0,
        }
    }
}

impl fmt::Display for CpuUsage<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CpuUsage {{ name: {}, usage: {} }}",
            self.name, self.usage
        )
    }
}

impl AsMetric for CpuUsage<'_> {
    type Output = f32;

    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<f32>> {
        let timestamp = current_timestamp();

        vec![Metric {
            //dev.localhost.cpu.processor1.percentage
            path: format!("{env}.{hostname}.cpu.{name}.percentage"),
            value: self.usage,
            timestamp,
        }]
    }
}

pub async fn cpu_metrics(env: &str, hostname: &str) -> Vec<MetricType> {
    let mut s =
        System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()));

    let _ = s.refresh_cpu_all();

    let cpu_metrics: Vec<_> = s
        .cpus()
        .iter()
        .map(|cpu| CpuUsage {
            name: cpu.name(),
            usage: round_to_two_decimal_places(cpu.cpu_usage()),
        })
        .into_iter()
        .map(|cpu| cpu.as_metric(cpu.name, env, hostname))
        .flatten()
        .collect();

    cpu_metrics
        .iter()
        .map(|metric| MetricType::F32(metric.clone()))
        .collect()
}
