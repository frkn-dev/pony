use std::collections::HashMap;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::{
    time::Instant,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

use base64::Engine;
use chrono::{Duration, Local, NaiveTime};
use chrono::{TimeZone, Utc};
use log::LevelFilter;
use rand::{distributions::Alphanumeric, Rng};
use tokio::time::{sleep, Duration as TokioDuration};

use crate::http::requests::InboundResponse;
use crate::state::Tag;
use crate::{PonyError, Result};

pub async fn run_daily<F, Fut>(task: F, target_time: NaiveTime)
where
    F: Fn() -> Fut + Send + Sync + 'static + Clone,
    Fut: std::future::Future<Output = ()> + Send,
{
    loop {
        log::debug!("Running daily scheduled task {}", target_time,);
        let now = Local::now();
        let today_target = now.date_naive().and_time(target_time);

        let next_run = {
            if now.time() < target_time {
                today_target
            } else {
                today_target + Duration::days(1)
            }
        };

        log::debug!(
            "Running daily shcduked task {}, next run - {}",
            today_target,
            next_run
        );

        let wait_duration = (next_run - now.naive_local()).to_std().unwrap_or_default();
        sleep(TokioDuration::from_secs(wait_duration.as_secs())).await;

        task().await;
    }
}

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

pub fn create_conn_link(
    tag: Tag,
    conn_id: &uuid::Uuid,
    inbound: InboundResponse,
    label: &str,
    address: Ipv4Addr,
) -> Result<String> {
    let raw_link = match tag {
        Tag::VlessXtls => vless_xtls_conn(conn_id, address, inbound.clone(), label),
        Tag::VlessGrpc => vless_grpc_conn(conn_id, address, inbound.clone(), label),
        Tag::Vmess => vmess_tcp_conn(conn_id, address, inbound.clone(), label),
        _ => return Err(PonyError::Custom("Cannot complete conn line".into()).into()),
    }?;

    let parsed =
        Url::parse(&raw_link).map_err(|_| PonyError::Custom("Invalid URL generated".into()))?;

    Ok(parsed.to_string())
}

fn vless_xtls_conn(
    conn_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
) -> Result<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings.ok_or(PonyError::Custom(
        "VLESS XTLS: stream settings error".into(),
    ))?;
    let reality_settings = stream_settings.reality_settings.ok_or(PonyError::Custom(
        "VLESS XTLS: reality settings error".into(),
    ))?;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first().ok_or(PonyError::Custom(
        "VLESS XTLS: reality settings SID error".into(),
    ))?;
    let sni = reality_settings
        .server_names
        .first()
        .ok_or(PonyError::Custom(
            "VLESS XTLS: reality settings SNI error".into(),
        ))?;

    Ok(format!(
        "vless://{conn_id}@{ipv4}:{port}?security=reality&flow=xtls-rprx-vision&type=tcp&sni={sni}&fp=chrome&pbk={pbk}&sid={sid}#{label} XTLS"
    ))
}

fn vless_grpc_conn(
    conn_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
) -> Result<String> {
    let port = inbound.port;
    let stream_settings = inbound.stream_settings.ok_or(PonyError::Custom(
        "VLESS GRPC: stream settings error".into(),
    ))?;
    let reality_settings = stream_settings.reality_settings.ok_or(PonyError::Custom(
        "VLESS GRPC: reality settings error".into(),
    ))?;
    let grpc_settings = stream_settings
        .grpc_settings
        .ok_or(PonyError::Custom("VLESS GRPC: grpc settings error".into()))?;
    let service_name = grpc_settings.service_name;
    let pbk = reality_settings.public_key;
    let sid = reality_settings.short_ids.first().ok_or(PonyError::Custom(
        "VLESS GRPC: reality settings SID error".into(),
    ))?;
    let sni = reality_settings
        .server_names
        .first()
        .ok_or(PonyError::Custom(
            "VLESS GRPC: reality settings SNI error".into(),
        ))?;

    let conn = format!(
        "vless://{conn_id}@{ipv4}:{port}?security=reality&type=grpc&mode=gun&serviceName={service_name}&fp=chrome&sni={sni}&pbk={pbk}&sid={sid}#{label} GRPC"
    );
    Ok(conn)
}

fn vmess_tcp_conn(
    conn_id: &uuid::Uuid,
    ipv4: Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
) -> Result<String> {
    let mut conn: HashMap<String, String> = HashMap::new();
    let port = inbound.port;
    let stream_settings = inbound
        .stream_settings
        .ok_or(PonyError::Custom("VMESS: stream settings error".into()))?;
    let tcp_settings = stream_settings
        .tcp_settings
        .ok_or(PonyError::Custom("VMESS: stream tcp settings error".into()))?;
    let header = tcp_settings
        .header
        .ok_or(PonyError::Custom("VMESS: header tcp settings error".into()))?;
    let req = header
        .request
        .ok_or(PonyError::Custom("VMESS: header req settings error".into()))?;
    let headers = req
        .headers
        .ok_or(PonyError::Custom("VMESS: headers settings error".into()))?;

    let host = headers
        .get("Host")
        .ok_or(PonyError::Custom("VMESS: host settings error".into()))?
        .first()
        .ok_or(PonyError::Custom(
            "VMESS: Host stream settings error".into(),
        ))?;
    let path = req
        .path
        .first()
        .ok_or(PonyError::Custom("VMESS: path settings error".into()))?;

    conn.insert("add".to_string(), ipv4.to_string());
    conn.insert("aid".to_string(), "0".to_string());
    conn.insert("host".to_string(), host.to_string());
    conn.insert("id".to_string(), conn_id.to_string());
    conn.insert("net".to_string(), "tcp".to_string());
    conn.insert("path".to_string(), path.to_string());
    conn.insert("port".to_string(), port.to_string());
    conn.insert("ps".to_string(), "VmessTCP".to_string());
    conn.insert("scy".to_string(), "auto".to_string());
    conn.insert("tls".to_string(), "none".to_string());
    conn.insert("type".to_string(), "http".to_string());
    conn.insert("v".to_string(), "2".to_string());

    let json_str = serde_json::to_string(&conn)
        .ok()
        .ok_or(PonyError::Custom("VMESS serde json error".into()))?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(json_str);

    Ok(format!("vmess://{base64_str}#{label} ____"))
}
