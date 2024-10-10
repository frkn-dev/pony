use log::info;
use std::{fmt, time};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use crate::config2::AppConfig;
use crate::metrics::{AsMetric, Metric};
use crate::utils::{current_timestamp, round_to_two_decimal_places, send_to_carbon};

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

    fn as_metric(&self, name: &str, settings: AppConfig) -> Vec<Metric<f32>> {
        let timestamp = current_timestamp();
        let h = &settings.hostname;
        let env = &settings.env;

        vec![Metric {
            path: format!("{env}.{h}.cpu_usage.{name}.percentage"),
            value: self.usage,
            timestamp,
        }]
    }
}

pub async fn cpu_metrics(server: String, settings: AppConfig) {
    info!("Starting cpu metric loop");

    loop {
        let mut s =
            System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));

        std::thread::sleep(time::Duration::from_secs(settings.metrics_delay));
        let _ = s.refresh_cpu_all();

        for cpu in s.cpus() {
            let metric = CpuUsage {
                name: cpu.name(),
                usage: round_to_two_decimal_places(cpu.cpu_usage()),
            };

            let _ =
                send_to_carbon(&metric.as_metric(cpu.name(), settings.clone())[0], &server).await;
        }
    }
}
