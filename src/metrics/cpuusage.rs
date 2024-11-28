use log::info;
use std::fmt;
use std::time::Duration;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tokio::time::sleep;

use crate::config2::Settings;
use crate::metrics::metrics::{AsMetric, Metric};
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

    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<f32>> {
        let timestamp = current_timestamp();
        let h = &settings.app.hostname;
        let env = &settings.app.env;

        vec![Metric {
            path: format!("{env}.{h}.cpu_usage.{name}.percentage"),
            value: self.usage,
            timestamp,
        }]
    }
}

pub async fn cpu_metrics(server: String, settings: Settings) {
    info!("Starting cpu metric loop");

    loop {
        let mut s =
            System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));

        sleep(Duration::from_secs(settings.app.metrics_delay)).await;

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
