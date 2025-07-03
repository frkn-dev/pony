use std::fmt;
use std::thread::sleep;
use std::time::Duration;

use log::error;
use sysinfo::Networks;

use super::metrics::{AsMetric, Metric, MetricType};
use crate::utils::current_timestamp;

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

    fn as_metric(&self, interface: &str, env: &str, hostname: &str) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();

        vec![
            Metric {
                //dev.localhost.network.eth0.rx_bps (bytes per second)
                metric: format!("{env}.{hostname}.network.{interface}.rx_bps"),
                value: self.rx_bps,
                timestamp,
            },
            Metric {
                //dev.localhost.network.eth0.tx_bps (bytes per second)
                metric: format!("{env}.{hostname}.network.{interface}.tx_bps"),
                value: self.tx_bps,
                timestamp,
            },
            Metric {
                //dev.localhost.network.eth0.rx_err
                metric: format!("{env}.{hostname}.network.{interface}.rx_err"),
                value: self.rx_err,
                timestamp,
            },
            Metric {
                //dev.localhost.network.eth0.tx_err
                metric: format!("{env}.{hostname}.network.{interface}.tx_err"),
                value: self.tx_err,
                timestamp,
            },
        ]
    }
}

pub fn bandwidth_metrics(env: &str, hostname: &str, target_interface: &str) -> Vec<MetricType> {
    let mut networks = Networks::new_with_refreshed_list();
    let _ = sleep(Duration::from_secs(1));

    let _ = networks.refresh(true);
    let res = networks
        .iter()
        .find(|&(interface, _)| interface == target_interface);

    match res {
        Some((interface, data)) => {
            let bandwidth = Bandwidth {
                rx_bps: data.received(),
                tx_bps: data.transmitted(),
                rx_err: data.errors_on_received(),
                tx_err: data.errors_on_transmitted(),
            };
            let bandwidth_metrics = bandwidth.as_metric(interface, env, hostname);

            bandwidth_metrics
                .iter()
                .map(|metric| MetricType::U64(metric.clone()))
                .collect()
        }
        None => {
            error!("Cannot find interface: {}", target_interface);
            vec![]
        }
    }
}
