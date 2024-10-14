use log::{error, info, warn, LevelFilter};
use std::error::Error;
use std::io;
use std::net::SocketAddr;

use chrono::{TimeZone, Utc};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::geoip;
use crate::metrics::Metric;
use std::net::IpAddr;

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

            info!("Sent metric to Carbon: {}", metric_string);

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

fn remove_prefix(input: &str) -> String {
    input.replace("::ffff:", "")
}

fn trim_quotes(s: &str) -> String {
    s.replace("\"", "").trim().to_string()
}

fn parse_ip(ip: &str) -> Result<String, Box<dyn Error + 'static>> {
    if let Ok(ip_addr) = ip.parse::<IpAddr>() {
        return Ok(ip_addr.to_string());
    }

    match ip.parse::<SocketAddr>() {
        Ok(socket_addr) => {
            let ip: IpAddr = socket_addr.ip();
            Ok(ip.to_string())
        }
        Err(_) => Err("Failed to parse IP or SocketAddr".into()),
    }
}

pub async fn country(ip: String) -> Result<String, Box<dyn Error>> {
    let parsed_ip = parse_ip(&ip)?;

    let country_info = geoip::find(&remove_prefix(&parsed_ip))
        .await
        .map_err(|_| "Failed to find country by IP")?;

    let country = trim_quotes(&country_info.country);

    Ok(country)
}
