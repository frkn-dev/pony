use chrono::{Duration, Local, NaiveTime};
use chrono::{TimeZone, Utc};
use defguard_wireguard_rs::net::IpAddrMask;
use ipnet::Ipv4Net;
use ipnet::Ipv6Net;
use log::LevelFilter;
use rand::{distributions::Alphanumeric, Rng};
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::time::Instant;
use tokio::time::{sleep, Duration as TokioDuration};
use url::Url;

use crate::http::requests::InboundResponse;
use crate::memory::tag::ProtoTag as Tag;
use crate::xray_op::vless::vless_grpc_conn;
use crate::xray_op::vless::vless_xtls_conn;
use crate::xray_op::vmess::vmess_tcp_conn;
use crate::{PonyError, Result};

pub fn increment_ip(ip: Ipv4Addr) -> Option<Ipv4Addr> {
    let ip_u32 = u32::from(ip);
    let next = ip_u32.checked_add(1)?;
    Some(Ipv4Addr::from(next))
}

pub fn ip_in_mask(mask: &IpAddrMask, addr: IpAddr) -> bool {
    match (mask.ip, mask.cidr, addr) {
        (IpAddr::V4(base), cidr, IpAddr::V4(addr)) => {
            if let Ok(net) = Ipv4Net::new(base, cidr) {
                net.contains(&addr)
            } else {
                false
            }
        }
        (IpAddr::V6(base), cidr, IpAddr::V6(addr)) => {
            if let Ok(net) = Ipv6Net::new(base, cidr) {
                net.contains(&addr)
            } else {
                false
            }
        }
        _ => false,
    }
}

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

pub fn to_pg_bigint(value: u64) -> Option<i64> {
    if value <= i64::MAX as u64 {
        Some(value as i64)
    } else {
        None
    }
}

pub fn from_pg_bigint(value: i64) -> u64 {
    value as u64
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

pub fn current_timestamp() -> i64 {
    Utc::now().timestamp()
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
        _ => return Err(PonyError::Custom("Cannot complete conn line".into())),
    }?;

    let parsed =
        Url::parse(&raw_link).map_err(|_| PonyError::Custom("Invalid URL generated".into()))?;

    Ok(parsed.to_string())
}
