use log::info;
use std::time::Duration;
use sysinfo::{LoadAvg, System};
use tokio::time::sleep;

use crate::config2::Settings;
use crate::metrics::{AsMetric, Metric};
use crate::utils::{current_timestamp, send_to_carbon};

struct LoadAvgWrapper {
    load_avg: LoadAvg,
}

impl AsMetric for LoadAvgWrapper {
    type Output = f64;

    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<f64>> {
        let timestamp = current_timestamp();
        let h = &settings.app.hostname;
        let env = &settings.app.env;

        vec![
            Metric {
                path: format!("{env}.{h}.{name}.1m"),
                value: self.load_avg.one,
                timestamp: timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.5m"),
                value: self.load_avg.five,
                timestamp: timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.15m"),
                value: self.load_avg.fifteen,
                timestamp: timestamp,
            },
        ]
    }
}

pub async fn loadavg_metrics(server: String, settings: Settings) {
    info!("Starting loadavg metric loop");

    loop {
        let load_avg = System::load_average();
        let wrapper = LoadAvgWrapper { load_avg };
        for metric in wrapper.as_metric("loadavg", settings.clone()) {
            let _ = send_to_carbon(&metric, &server).await;
        }
        sleep(Duration::from_secs(settings.app.metrics_delay)).await;
    }
}
