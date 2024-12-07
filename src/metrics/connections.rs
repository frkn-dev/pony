use log::{debug, info};
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use std::time::Duration;
use std::{collections::HashMap, collections::HashSet};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use crate::appconfig::Settings;
use crate::metrics::metrics::{AsMetric, Metric};
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

    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();
        let h = &settings.app.hostname;
        let env = &settings.app.env;

        vec![
            Metric {
                path: format!("{env}.{h}.{name}.vmess"),
                value: self.vmess,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.vless"),
                value: self.vless,
                timestamp,
            },
            Metric {
                path: format!("{env}.{h}.{name}.ss"),
                value: self.ss,
                timestamp,
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct WgConnections {
    connections: u64,
}

impl AsMetric for WgConnections {
    type Output = u64;

    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<u64>> {
        let timestamp = current_timestamp();
        let h = &settings.app.hostname;
        let env = &settings.app.env;

        vec![Metric {
            path: format!("{env}.{h}.{name}.wg"),
            value: self.connections,
            timestamp,
        }]
    }
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
            ProtocolSocketInfo::Udp(udp_si) => {
                if udp_si.local_port == target_port {
                    connections.insert(ConnectionInfo {
                        local_addr: udp_si.local_addr.to_string(),
                        local_port: udp_si.local_port,
                        remote_addr: "null".to_string(),
                        remote_port: 0,
                        state: "UDP".to_string(),
                    });
                }
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
    settings: Settings,
    data: HashSet<ConnectionInfo>,
    name: &str,
    server: String,
) {
    let env = settings.app.env.clone();
    let hostname = settings.app.hostname.clone();

    let country_data = country_parse(data);

    for (country, count) in country_data.await {
        let metric = Metric::new(
            format!("{env}.{hostname}.{name}.connections.geo.{country}"),
            count,
            current_timestamp(),
        );

        info!("Metric {}", metric.to_string());

        if let Err(e) = send_to_carbon(&metric, &server).await {
            log::error!("Failed to send metric to Carbon: {}", e);
        }
    }
}

pub async fn connections_metric(server: String, settings: Settings) {
    info!("Starting connections metric loop");

    loop {
        let mut tasks: Vec<JoinHandle<()>> = vec![];

        if settings.xray.enabled {
            let vmess_connections = port_connections_data(settings.xray.vmess_port);
            let vless_connections = port_connections_data(settings.xray.vless_port);
            let ss_connections = port_connections_data(settings.xray.ss_port);

            let xray_connections = XrayConnections {
                vmess: vmess_connections.len() as u64,
                vless: vless_connections.len() as u64,
                ss: ss_connections.len() as u64,
            };

            for metric in xray_connections.as_metric("connections", settings.clone()) {
                if let Err(e) = send_to_carbon(&metric, &server).await {
                    log::error!("Failed to send connection metric: {}", e);
                }
            }

            tasks.push(tokio::spawn(send_to_carbon_country_metric(
                settings.clone(),
                vmess_connections.clone(),
                "vmess",
                server.clone(),
            )));
            tasks.push(tokio::spawn(send_to_carbon_country_metric(
                settings.clone(),
                vless_connections.clone(),
                "vless",
                server.clone(),
            )));
            tasks.push(tokio::spawn(send_to_carbon_country_metric(
                settings.clone(),
                ss_connections.clone(),
                "ss",
                server.clone(),
            )));
        }

        if settings.wg.enabled {
            let wg_connections_data = port_connections_data(settings.wg.port);
            let wg_connections = WgConnections {
                connections: wg_connections_data.len() as u64,
            };
            for metric in wg_connections.as_metric("connections", settings.clone()) {
                if let Err(e) = send_to_carbon(&metric, &server).await {
                    log::error!("Failed to send connection metric: {}", e);
                }
            }
        }

        let _ = futures::future::try_join_all(tasks).await;

        sleep(Duration::from_secs(settings.app.metrics_delay)).await;
    }
}
