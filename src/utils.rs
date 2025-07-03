use base64::Engine;
use chrono::{Duration, Local, NaiveTime};
use chrono::{TimeZone, Utc};
use defguard_wireguard_rs::net::IpAddrMask;
use ipnet::Ipv4Net;
use ipnet::Ipv6Net;
use log::LevelFilter;
use rand::{distributions::Alphanumeric, Rng};
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::time::Instant;
use tokio::time::{sleep, Duration as TokioDuration};
use url::Url;

use crate::http::requests::InboundResponse;
use crate::state::tag::Tag;
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
        _ => return Err(PonyError::Custom("Cannot complete conn line".into()).into()),
    }?;

    let parsed =
        Url::parse(&raw_link).map_err(|_| PonyError::Custom("Invalid URL generated".into()))?;

    Ok(parsed.to_string())
}

pub fn wireguard_conn(
    conn_id: &Uuid,
    ipv4: &Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
    private_key: &str,
    client_ip: &IpAddrMask,
) -> Result<String> {
    if let Some(wg) = inbound.wg {
        let server_pubkey = wg.pubkey;
        let host = ipv4;
        let port = wg.port;

        let config = format!(
            "[Interface]\n\
         PrivateKey = {private_key}\n\
         Address    = {client_ip}\n\
         DNS        = 1.1.1.1\n\
         \n\
         [Peer]\n\
         PublicKey           = {server_pubkey}\n\
         Endpoint            = {host}:{port}\n\
         AllowedIPs          = 0.0.0.0/0, ::/0\n\
         PersistentKeepalive = 25\n\
         \n\
         # {label} — conn_id: {conn_id}\n"
        );

        Ok(config)
    } else {
        Err(PonyError::Custom("WG is not configured".into()))
    }
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
    conn.insert("ps".to_string(), format!("Vmess {}", label));
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

use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ClashConfig {
    port: u16,
    mode: &'static str,
    proxies: Vec<ClashProxy>,
    #[serde(rename = "proxy-groups")]
    proxy_groups: Vec<ClashProxyGroup>,
    rules: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum ClashProxy {
    Vmess {
        name: String,
        server: String,
        port: u16,
        uuid: String,
        cipher: String,
        udp: bool,
        #[serde(rename = "alterId")]
        alter_id: u8,
        network: String,
        #[serde(rename = "http-opts")]
        http_opts: HttpOpts,
    },
    Vless {
        name: String,
        server: String,
        port: u16,
        uuid: String,
        udp: bool,
        tls: bool,
        network: String,
        servername: String,
        #[serde(rename = "reality-opts")]
        reality_opts: RealityOpts,
        #[serde(rename = "grpc-opts", skip_serializing_if = "Option::is_none")]
        grpc_opts: Option<GrpcOpts>,
        #[serde(rename = "client-fingerprint")]
        client_fingerprint: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        flow: Option<String>,
    },
}

#[derive(Serialize)]
pub struct HttpOpts {
    method: &'static str,
    path: Vec<String>,
    headers: HttpHeaders,
    #[serde(rename = "ip-version")]
    ip_version: &'static str,
    host: String,
}

#[derive(Serialize)]
pub struct HttpHeaders {
    connection: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct RealityOpts {
    #[serde(rename = "public-key")]
    public_key: String,
    #[serde(rename = "short-id")]
    short_id: String,
}

#[derive(Serialize)]
pub struct GrpcOpts {
    #[serde(rename = "grpc-service-name")]
    grpc_service_name: String,
    #[serde(rename = "ip-version")]
    ip_version: &'static str,
}

#[derive(Serialize)]
pub struct ClashProxyGroup {
    name: String,
    #[serde(rename = "type")]
    group_type: String,
    url: String,
    interval: u32,
    proxies: Vec<String>,
}

pub fn generate_proxy_config(
    inbound: &InboundResponse,
    conn_id: Uuid,
    address: Ipv4Addr,
    label: &str,
) -> Option<ClashProxy> {
    let port = inbound.port;
    let stream = inbound.stream_settings.as_ref()?;

    let proxy = match inbound.tag {
        Tag::Vmess => {
            let tcp = stream.tcp_settings.as_ref()?;
            let header = tcp.header.as_ref()?;
            let req = header.request.as_ref()?;
            let host = req.headers.as_ref()?.get("Host")?.get(0)?.clone();
            let path = req.path.first().cloned().unwrap_or_else(|| "/".to_string());

            let name = format!("{} [{}]", label, inbound.tag.to_string());

            Some(ClashProxy::Vmess {
                name,
                server: address.to_string(),
                port,
                uuid: conn_id.to_string(),
                cipher: "auto".to_string(),
                udp: true,
                alter_id: 0,
                network: "http".to_string(),
                http_opts: HttpOpts {
                    method: "GET",
                    path: vec![path],
                    headers: HttpHeaders {
                        connection: vec!["keep-alive"],
                    },
                    ip_version: "dual",
                    host,
                },
            })
        }
        Tag::VlessGrpc | Tag::VlessXtls => {
            let reality = stream.reality_settings.as_ref()?;

            let (network, grpc_opts, flow) = if let Some(grpc) = &stream.grpc_settings {
                (
                    "grpc".to_string(),
                    Some(GrpcOpts {
                        grpc_service_name: grpc.service_name.clone(),
                        ip_version: "dual",
                    }),
                    None,
                )
            } else {
                (
                    "tcp".to_string(),
                    None,
                    Some("xtls-rprx-vision".to_string()),
                )
            };

            let name = format!("{} [{}]", label, inbound.tag);

            Some(ClashProxy::Vless {
                name,
                server: address.to_string(),
                port,
                uuid: conn_id.to_string(),
                udp: true,
                tls: true,
                network,
                servername: reality.server_names.get(0).cloned().unwrap_or_default(),
                client_fingerprint: "chrome".to_string(),
                reality_opts: RealityOpts {
                    public_key: reality.public_key.clone(),
                    short_id: reality.short_ids.get(0).cloned().unwrap_or_default(),
                },
                grpc_opts,
                flow,
            })
        }
        _ => return None,
    };

    proxy
}

pub fn generate_clash_config(proxies: Vec<ClashProxy>) -> ClashConfig {
    let proxy_names = proxies
        .iter()
        .map(|proxy| match proxy {
            ClashProxy::Vmess { name, .. } => name.clone(),
            ClashProxy::Vless { name, .. } => name.clone(),
        })
        .collect();

    ClashConfig {
        port: 7890,
        mode: "global",
        proxies,
        proxy_groups: vec![ClashProxyGroup {
            name: "♻️ Automatic".into(),
            group_type: "url-test".into(),
            url: "http://www.gstatic.com/generate_204".into(),
            interval: 300,
            proxies: proxy_names,
        }],
        rules: vec![],
    }
}
