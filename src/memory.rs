use log::info;
use std::{thread, time};
use sysinfo::{MemoryRefreshKind, System};

use crate::config2::AppConfig;
use crate::metrics::{AsMetric, Metric};
use crate::utils::{current_timestamp, send_to_carbon};

struct MemUsage {
    free: u64,
    total: u64,
}

impl AsMetric for MemUsage {
    type Output = u64;
    fn as_metric(&self, name: &str, settings: AppConfig) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();
        let h = &settings.hostname;
        let env = &settings.env;

        vec![
            Metric {
                path: format!("{env}.{h}.{name}.total"),
                value: self.total,
                timestamp: timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.free"),
                value: self.free,
                timestamp: timestamp,
            },
        ]
    }
}

pub async fn mem_metrics(server: String, settings: AppConfig) {
    info!("Starting memory metric loop");

    loop {
        let mut system = System::new();

        let _ = system.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
        let mem_metrics = MemUsage {
            free: system.free_memory(),
            total: system.total_memory(),
        };

        for metric in mem_metrics.as_metric("mem", settings.clone()) {
            let _ = send_to_carbon(&metric, &server).await;
        }

        thread::sleep(time::Duration::from_secs(settings.metrics_delay));
    }
}
