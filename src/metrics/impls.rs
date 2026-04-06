use chrono::Utc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use sysinfo::Networks;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use super::storage::HasMetrics;
use super::storage::MetricSink;
use super::Metrics;

static BEAT_INDEX: OnceLock<AtomicUsize> = OnceLock::new();

impl<T> Metrics for T
where
    T: HasMetrics + Sync + Send,
{
    async fn heartbeat(&self) {
        let timestamp = Utc::now().timestamp();

        let pattern: &[f64] = &[
            1.0, 1.0, 2.5, 0.4, 0.6, 1.3, 1.0, 1.0, 1.1, 1.0, 1.0, 0.8, 1.0, 1.9, 0.3, 0.5, 1.0,
        ];
        let value = {
            let index = BEAT_INDEX
                .get_or_init(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::Relaxed)
                % pattern.len();
            pattern[index]
        };

        let node_id = self.node_id();
        self.metrics().write(node_id, "heartbeat", value, timestamp);
        log::debug!("Stored heartbeat metric: {}", value);
    }

    async fn memory(&self) {
        let timestamp = chrono::Utc::now().timestamp();

        let mut system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
        );
        system.refresh_memory();

        let free = system.free_memory() as f64;
        let total = system.total_memory() as f64;
        let used = system.used_memory() as f64;
        let available = system.available_memory() as f64;

        let node_id = self.node_id();

        self.metrics().write(node_id, "mem_free", free, timestamp);
        self.metrics().write(node_id, "mem_total", total, timestamp);
        self.metrics().write(node_id, "mem_used", used, timestamp);
        self.metrics()
            .write(node_id, "mem_available", available, timestamp);

        log::debug!(
            "Stored memory metrics: free={}, total={}, used={}, available={}",
            free,
            total,
            used,
            available
        );
    }
    async fn loadavg(&self) {
        let timestamp = Utc::now().timestamp();

        let load = System::load_average();

        let node_id = self.node_id();

        self.metrics()
            .write(node_id, "loadavg_1", load.one, timestamp);
        self.metrics()
            .write(node_id, "loadavg_5", load.five, timestamp);
        self.metrics()
            .write(node_id, "loadavg_15", load.fifteen, timestamp);

        log::debug!(
            "Stored loadavg metrics: 1min={}, 5min={}, 15min={}",
            load.one,
            load.five,
            load.fifteen
        );
    }
    async fn bandwidth(&self) {
        let now = Utc::now().timestamp();
        let mut networks = Networks::new_with_refreshed_list();
        std::thread::sleep(std::time::Duration::from_secs(1));
        networks.refresh(true);

        for (interface, data) in networks.iter() {
            log::debug!("Storing bandwidth metrics for interface {}", interface);
            let node_id = self.node_id();

            self.metrics().write(
                node_id,
                &format!("network.{interface}.total_rx_bps"),
                data.total_received() as f64,
                now,
            );

            self.metrics().write(
                node_id,
                &format!("network.{interface}.total_tx_bps"),
                data.total_transmitted() as f64,
                now,
            );

            self.metrics().write(
                node_id,
                &format!("network.{interface}.rx_bps"),
                data.received() as f64,
                now,
            );
            self.metrics().write(
                node_id,
                &format!("network.{interface}.tx_bps"),
                data.transmitted() as f64,
                now,
            );
            self.metrics().write(
                node_id,
                &format!("network.{interface}.rx_err"),
                data.errors_on_received() as f64,
                now,
            );
            self.metrics().write(
                node_id,
                &format!("network.{interface}.tx_err"),
                data.errors_on_transmitted() as f64,
                now,
            );
        }
    }
    async fn cpu_usage(&self) {
        let now = Utc::now().timestamp();

        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );

        sys.refresh_cpu_all();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        sys.refresh_cpu_all();

        for cpu in sys.cpus() {
            let usage = (cpu.cpu_usage() * 100.0).round() / 100.0;
            let metric_name = format!("cpu.{}", cpu.name());
            let node_id = self.node_id();

            log::debug!("Storing CPU metric {} = {}", metric_name, usage);

            self.metrics()
                .write(node_id, &metric_name, usage as f64, now);
        }
    }
}
