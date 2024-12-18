use log::info;
use std::time::Duration;
use sysinfo::{MemoryRefreshKind, System};
use tokio::time::sleep;

use crate::{
    metrics::metrics::{AsMetric, Metric},
    settings::Settings,
    utils::{current_timestamp, send_to_carbon},
};

struct MemUsage {
    free: u64,
    total: u64,
}

impl AsMetric for MemUsage {
    type Output = u64;
    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();
        if let Some(hostname) = settings.node.hostname {
            let env = &settings.node.env;

            vec![
                Metric {
                    path: format!("{env}.{hostname}.{name}.total"),
                    value: self.total,
                    timestamp: timestamp,
                },
                Metric {
                    path: format!("{env}.{hostname}.{name}.free"),
                    value: self.free,
                    timestamp: timestamp,
                },
            ]
        } else {
            vec![]
        }
    }
}

pub async fn mem_metrics(server: String, settings: Settings) {
    info!("Starting memory metric loop");

    loop {
        let mut system = System::new();

        let _ = system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        let mem_metrics = MemUsage {
            free: system.free_memory(),
            total: system.total_memory(),
        };

        for metric in mem_metrics.as_metric("mem", settings.clone()) {
            let _ = send_to_carbon(&metric, &server).await;
        }

        sleep(Duration::from_secs(settings.app.metrics_delay)).await;
    }
}
