use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{TimeZone, Utc};
use log::{debug, error, warn, LevelFilter};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use super::metrics::metrics::Metric;

pub async fn send_to_carbon<T: ToString>(
    metric: &Metric<T>,
    server: &str,
) -> Result<(), io::Error> {
    let metric_string = metric.to_string();

    match TcpStream::connect(server).await {
        Ok(mut stream) => {
            if let Err(e) = stream.write_all(metric_string.as_bytes()).await {
                warn!("Failed to send metric: {}", e);
                return Err(e);
            }

            debug!("Sent metric to Carbon: {}", metric_string);

            if let Err(e) = stream.flush().await {
                warn!("Failed to flush stream: {}", e);
                return Err(e);
            }

            Ok(())
        }
        Err(e) => {
            error!("Failed to connect to Carbon server: {}", e);
            Err(e)
        }
    }
}

pub fn current_timestamp() -> u64 {
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

pub fn human_readable_date(timestamp: u64) -> String {
    match Utc.timestamp_opt(timestamp as i64, 0) {
        chrono::LocalResult::Single(datetime) => datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => "Invalid timestamp".to_string(),
    }
}

pub fn round_to_two_decimal_places(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

pub fn level_from_settings(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}
