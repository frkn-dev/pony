use std::collections::HashMap;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::{
    time::Instant,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::Engine;
use chrono::{TimeZone, Utc};
use log::LevelFilter;
use rand::{distributions::Alphanumeric, Rng};

use crate::config::xray::Inbound;

pub fn generate_random_password(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub fn to_ipv4(ip: IpAddr) -> Option<Ipv4Addr> {
    match ip {
        IpAddr::V4(ipv4) => Some(ipv4),
        IpAddr::V6(_) => None,
    }
}

pub async fn measure_time<T, F>(task: F, name: String) -> T
where
    F: std::future::Future<Output = T>,
{
    let start_time = Instant::now();
    let result = task.await;
    let duration = start_time.elapsed();
    log::info!("Task {} completed in {:?}", name, duration);
    result
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

pub fn vless_xtls_conn(
    user_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: Inbound,
    label: String,
) -> Option<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings?;
    let reality_settings = stream_settings.reality_settings?;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first()?;
    let sni = reality_settings.server_names.first()?;

    let conn = format!(
        "vless://{user_id}@{ipv4}:{port}?security=reality&flow=xtls-rprx-vision&type=tcp&sni={sni}&fp=chrome&pbk={pbk}&sid={sid}#{label} XTLS"
    );
    Some(conn)
}

pub fn vless_grpc_conn(
    user_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: Inbound,
    label: String,
) -> Option<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings?;
    let reality_settings = stream_settings.reality_settings?;
    let grpc_settings = stream_settings.grpc_settings?;
    let service_name = grpc_settings.service_name;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first()?;
    let sni = reality_settings.server_names.first()?;

    let conn = format!(
        "vless://{user_id}@{ipv4}:{port}?security=reality&type=grpc&mode=gun&serviceName={service_name}&fp=chrome&sni={sni}&pbk={pbk}&sid={sid}#{label} GRPC"
    );
    Some(conn)
}

pub fn vmess_tcp_conn(user_id: &uuid::Uuid, ipv4: Ipv4Addr, inbound: Inbound) -> Option<String> {
    let mut conn: HashMap<String, String> = HashMap::new();
    let port = inbound.port;
    let stream_settings = inbound.stream_settings?;
    let tcp_settings = stream_settings.tcp_settings?;
    let header = tcp_settings.header?;
    let req = header.request?;
    let headers = req.headers?;

    let host = headers.get("Host")?.first()?;
    let path = req.path.first()?;

    conn.insert("add".to_string(), ipv4.to_string());
    conn.insert("aid".to_string(), "0".to_string());
    conn.insert("host".to_string(), host.to_string());
    conn.insert("id".to_string(), user_id.to_string());
    conn.insert("net".to_string(), "tcp".to_string());
    conn.insert("path".to_string(), path.to_string());
    conn.insert("port".to_string(), port.to_string());
    conn.insert("ps".to_string(), "VmessTCP".to_string());
    conn.insert("scy".to_string(), "auto".to_string());
    conn.insert("tls".to_string(), "none".to_string());
    conn.insert("type".to_string(), "http".to_string());
    conn.insert("v".to_string(), "2".to_string());

    let json_str = serde_json::to_string(&conn).ok()?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(json_str);

    Some(format!("vmess://{base64_str}"))
}
