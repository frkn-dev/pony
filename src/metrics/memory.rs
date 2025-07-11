use sysinfo::{MemoryRefreshKind, System};

use super::metrics::{AsMetric, Metric, MetricType};
use crate::utils::current_timestamp;

struct MemUsage {
    free: u64,
    total: u64,
    used: u64,
    available: u64,
}

impl AsMetric for MemUsage {
    type Output = u64;
    fn as_metric(&self, name: &str, env: &str, hostname: &str) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();

        vec![
            Metric {
                //dev.localhost.mem.total
                metric: format!("{env}.{hostname}.{name}.total"),
                value: self.total,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.mem.free
                metric: format!("{env}.{hostname}.{name}.free"),
                value: self.free,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.mem.used
                metric: format!("{env}.{hostname}.{name}.used"),
                value: self.used,
                timestamp: timestamp,
            },
            Metric {
                //dev.localhost.mem.available
                metric: format!("{env}.{hostname}.{name}.available"),
                value: self.available,
                timestamp: timestamp,
            },
        ]
    }
}

pub fn mem_metrics(env: &str, hostname: &str) -> Vec<MetricType> {
    let mut system = System::new();

    let _ = system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let mem = MemUsage {
        free: system.free_memory(),
        total: system.total_memory(),
        used: system.used_memory(),
        available: system.available_memory(),
    };

    let mem_metrics = mem.as_metric("mem", env, hostname);

    mem_metrics
        .iter()
        .map(|metric| MetricType::U64(metric.clone()))
        .collect()
}
