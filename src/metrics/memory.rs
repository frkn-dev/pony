use sysinfo::{MemoryRefreshKind, System};

use crate::{
    metrics::metrics::{AsMetric, Metric, MetricType},
    utils::current_timestamp,
};

struct MemUsage {
    free: u64,
    total: u64,
    used: u64,
}

impl AsMetric for MemUsage {
    type Output = u64;
    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();

        vec![
            Metric {
                //dev.localhost.mem.total
                path: format!("{env}.{hostname}.{name}.total"),
                value: self.total,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.mem.free
                path: format!("{env}.{hostname}.{name}.free"),
                value: self.free,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.mem.used
                path: format!("{env}.{hostname}.{name}.used"),
                value: self.used,
                timestamp: timestamp,
            },
        ]
    }
}

pub async fn mem_metrics(env: &str, hostname: &str) -> Vec<MetricType> {
    let mut system = System::new();

    let _ = system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let mem = MemUsage {
        free: system.free_memory(),
        total: system.total_memory(),
        used: system.used_memory(),
    };

    let mem_metrics = mem.as_metric("mem", env, hostname);

    mem_metrics
        .iter()
        .map(|metric| MetricType::U64(metric.clone()))
        .collect()
}
