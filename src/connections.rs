use log::{debug, info};
use std::time::Duration;
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use std::{collections::HashMap, collections::HashSet};
use tokio::time::sleep;

use crate::config2::AppConfig;
use crate::metrics::{AsMetric, Metric};
use crate::utils::{country, current_timestamp, send_to_carbon};

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct ConnectionInfo {
    local_addr: String,
    local_port: u16,
    remote_addr: String,
    remote_port: u16,
    state: String,
}

#[derive(Clone, Debug)]
struct XrayConnections {
    vmess: u64,
    vless: u64,
    ss: u64,
}

impl AsMetric for XrayConnections {
    type Output = u64;

    fn as_metric(&self, name: &str, settings: AppConfig) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();
        let h = &settings.hostname;
        let env = &settings.env;

        vec![
            Metric {
                path: format!("{env}.{h}.{name}.vmess.connections"),
                value: self.vmess,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.vless.connections"),
                value: self.vless,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.ss.connections"),
                value: self.ss,
                timestamp,
            },
        ]
    }
}

fn filter_unique_by_remote_addr(connections: &HashSet<ConnectionInfo>) -> HashSet<ConnectionInfo> {
    let mut seen_remote_addrs = HashSet::new();

    connections
        .iter()
        .filter(|conn| seen_remote_addrs.insert(&conn.remote_addr))
        .cloned()
        .collect()
}

fn port_connections_data(target_port: u16) -> HashSet<ConnectionInfo> {
    let mut connections = HashSet::new();

    debug!("Run {target_port} connections");

    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;
    let sockets_info = get_sockets_info(af_flags, proto_flags).unwrap();

    for si in sockets_info {
        match si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp_si) => {
                if tcp_si.local_port == target_port && tcp_si.state == TcpState::Established {
                    connections.insert(ConnectionInfo {
                        local_addr: tcp_si.local_addr.to_string(),
                        local_port: tcp_si.local_port,
                        remote_addr: tcp_si.remote_addr.to_string(),
                        remote_port: tcp_si.remote_port,
                        state: format!("{:?}", tcp_si.state),
                    });
                }
            }
            ProtocolSocketInfo::Udp(_) => {
                // Skip UDP
            }
        }
    }
    return connections;
}

async fn country_parse(connections: HashSet<ConnectionInfo>) -> HashMap<String, u64> {
    let mut country_connections: HashMap<String, u64> = HashMap::new();

    for connection in connections {
        if let Ok(country) = country(connection.remote_addr).await {
            *country_connections.entry(country.clone()).or_insert(0) += 1;
        }
    }
    country_connections
}

async fn send_to_carbon_country_metric(
    settings: AppConfig,
    data: HashSet<ConnectionInfo>,
    name: &str,
    server: &str,
) {
    let env = settings.env.clone();
    let hostname = settings.hostname.clone();

    let country_data = country_parse(data);

    for (country, count) in country_data.await {
        let metric = Metric::new(
            format!("{env}.{hostname}.{name}.geo.{country}.connections"),
            count,
            current_timestamp(),
        );

        info!("Metric {}", metric.to_string());

        if let Err(e) = send_to_carbon(&metric, server).await {
            log::error!("Failed to send metric to Carbon: {}", e);
        }
    }
}

pub async fn connections_metric(server: String, settings: AppConfig) {
    info!("Starting connections metric loop");

    loop {
        let vmess_connections = port_connections_data(settings.xray_vmess_port);
        let unique_vmess_connections = filter_unique_by_remote_addr(&vmess_connections);

        let vless_data_connections = port_connections_data(settings.xray_vless_port);
        let unique_vless_connections = filter_unique_by_remote_addr(&vless_data_connections);

        let ss_data_connections = port_connections_data(settings.xray_ss_port);
        let unique_ss_connections = filter_unique_by_remote_addr(&ss_data_connections);

        let xray_connections = XrayConnections {
            vmess: unique_vmess_connections.len() as u64,
            vless: unique_vless_connections.len() as u64,
            ss: unique_ss_connections.len() as u64,
        };

        for metric in xray_connections.as_metric("xray", settings.clone()) {
            if let Err(e) = send_to_carbon(&metric, &server).await {
                log::error!("Failed to send connection metric: {}", e);
            }
        }

        let vmess_task = send_to_carbon_country_metric(
            settings.clone(),
            unique_vmess_connections,
            "vmess",
            &server,
        );
        let vless_task = send_to_carbon_country_metric(
            settings.clone(),
            unique_vless_connections,
            "vless",
            &server,
        );
        let ss_task = send_to_carbon_country_metric(
            settings.clone(),
            unique_ss_connections,
            "ss",
            &server,
        );
        tokio::join!(vmess_task, vless_task, ss_task);
        sleep(Duration::from_secs(settings.metrics_delay)).await;
    }
}
