use log::info;
use std::fmt;
use std::time::Duration;
use sysinfo::Networks;
use tokio::time::sleep;

use crate::{
    metrics::metrics::{AsMetric, Metric},
    settings::Settings,
    utils::{current_timestamp, send_to_carbon},
};

#[derive(Debug)]
struct Bandwidth {
    rx_bps: u64,
    tx_bps: u64,
    rx_err: u64,
    tx_err: u64,
}

impl Default for Bandwidth {
    fn default() -> Self {
        Bandwidth {
            rx_bps: 0,
            tx_bps: 0,
            rx_err: 0,
            tx_err: 0,
        }
    }
}

impl fmt::Display for Bandwidth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Bandwidth {{ rx_bps: {}, tx_bps: {}, rx_err: {}, tx_err: {} }}",
            self.rx_bps, self.tx_bps, self.rx_err, self.tx_err
        )
    }
}

impl AsMetric for Bandwidth {
    type Output = u64;

    fn as_metric(&self, interface: &str, settings: Settings) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();
        let h = &settings.app.hostname;
        let env = &settings.app.env;

        vec![
            Metric {
                path: format!("{env}.{h}.network.{interface}.rx_bps"),
                value: self.rx_bps,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.network.{interface}.tx_bps"),
                value: self.tx_bps,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.network.{interface}.rx_err"),
                value: self.rx_err,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.network.{interface}.tx_err"),
                value: self.tx_err,
                timestamp,
            },
        ]
    }
}

pub async fn bandwidth_metrics(server: String, settings: Settings) {
    info!("Starting bandwidth metric loop");
    let mut networks = Networks::new_with_refreshed_list();

    loop {
        let _ = networks.refresh(true);
        let res = networks
            .iter()
            .find(|&(interface, _)| interface == &settings.app.iface);

        match res {
            Some((interface, data)) => {
                let bandwidth = Bandwidth {
                    rx_bps: data.packets_received(),
                    tx_bps: data.packets_transmitted(),
                    rx_err: data.errors_on_received(),
                    tx_err: data.errors_on_transmitted(),
                };

                let metrics = bandwidth.as_metric(interface, settings.clone());
                for metric in metrics {
                    let _ = send_to_carbon(&metric, &server).await;
                }
            }
            None => println!("Interface '{}' not found.", &settings.app.iface),
        }
        sleep(Duration::from_secs(settings.app.metrics_delay)).await;
    }
}
