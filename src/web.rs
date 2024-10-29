use actix_web::web::{Data, Path};
use actix_web::{get, HttpResponse, Responder};
use clickhouse::Client;
use log::debug;
use log::error;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::clickhouse::fetch_metrics_value;

#[derive(Deserialize)]
struct Params {
    env: String,
    cluster: String,
}

#[derive(Serialize, Debug, Clone)]
struct Connection {
    #[serde(flatten)]
    connection: HashMap<String, f64>,
}

#[derive(Serialize, Debug, Clone)]
struct ConnectionsByType {
    vless: Vec<Connection>,
    vmess: Vec<Connection>,
    ss: Vec<Connection>,
    wg: Vec<Connection>,
}

#[derive(Serialize)]
struct StatusResponse {
    connections: ConnectionsByServer,
    mbps: MbpsByServer,
}

type Bps = HashMap<String, f64>;
type MbpsByServer = Vec<HashMap<String, HashMap<String, f64>>>;

fn convert_bps_to_mbps(rx: Bps, tx: Bps) -> MbpsByServer {
    let mut server_list: MbpsByServer = Vec::new();

    for (server, &rx_value) in &rx {
        let tx_value = tx.get(server).copied().unwrap_or(0.0);
        let mut server_entry = HashMap::new();
        server_entry.insert(
            server.clone(),
            HashMap::from([
                ("rx".to_string(), (rx_value * 1500.0 / 1024.0 / 1024.0)),
                ("tx".to_string(), (tx_value * 1500.0 / 1024.0 / 1024.0)),
            ]),
        );
        server_list.push(server_entry);
    }

    server_list
}

type ConnectionsByServer = HashMap<String, Vec<HashMap<String, f64>>>;

fn convert_connections(connections: ConnectionsByType) -> ConnectionsByServer {
    let mut server_map: ConnectionsByServer = HashMap::new();

    for (conn_type, conn_list) in [
        ("vless", connections.vless),
        ("vmess", connections.vmess),
        ("ss", connections.ss),
        ("wg", connections.wg),
    ] {
        for conn in conn_list {
            for (server, &value) in &conn.connection {
                server_map
                    .entry(server.clone())
                    .or_insert_with(Vec::new)
                    .push(HashMap::from([(conn_type.to_string(), value)]));
            }
        }
    }

    server_map
}

pub async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().body("404 - Not Found")
}

#[get("/")]
pub async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello, Fuckin World!")
}

#[get("/status/{env}/{cluster}")]
pub async fn status(req: Path<Params>, ch_client: Data<Arc<Client>>) -> impl Responder {
    let mut connections_by_type = ConnectionsByType {
        vless: Vec::new(),
        vmess: Vec::new(),
        ss: Vec::new(),
        wg: Vec::new(),
    };

    let connection_types = ["vless", "vmess", "ss", "wg"];

    for connection_type in connection_types {
        let request_postfix = format!("connections.{connection_type}");
        let metrics_connections =
            fetch_metrics_value(&ch_client, &req.env, &req.cluster, &request_postfix).await;

        let metrics_value_connections = match metrics_connections {
            Ok(metrics) => metrics
                .into_iter()
                .map(|m| (m.metric.clone(), m.value.clone()))
                .collect::<HashMap<_, _>>(),
            Err(err) => {
                error!("Couldn't get metrics {}", err);
                HashMap::new()
            }
        };

        for (metric, value) in metrics_value_connections {
            let parts: Vec<&str> = metric.split('.').collect();
            if parts.len() == 4 {
                let server = parts[1].to_string();
                let mut connection_map = HashMap::new();
                connection_map.insert(server, value);

                match parts[3] {
                    "vless" => connections_by_type.vless.push(Connection {
                        connection: connection_map,
                    }),
                    "vmess" => connections_by_type.vmess.push(Connection {
                        connection: connection_map,
                    }),
                    "ss" => connections_by_type.ss.push(Connection {
                        connection: connection_map,
                    }),
                    "wg" => connections_by_type.wg.push(Connection {
                        connection: connection_map,
                    }),
                    _ => {}
                }
            }
        }
    }

    let metrics_bps = fetch_metrics_value(&ch_client, &req.env, &req.cluster, "%bps").await;
    let metrics_value_bps = match metrics_bps {
        Ok(metrics) => metrics
            .into_iter()
            .map(|m| (m.metric.clone(), m.value.clone()))
            .collect::<HashMap<_, _>>(),
        Err(err) => {
            error!("Couldn't get metrics {}", err);
            HashMap::new()
        }
    };

    let mut rx: HashMap<String, f64> = HashMap::new();
    let mut tx: HashMap<String, f64> = HashMap::new();

    for (metric, value) in metrics_value_bps {
        if let Some(stripped_metric) = metric.strip_prefix(format!("{}.", req.env).as_str()) {
            let parts: Vec<&str> = stripped_metric.split('.').collect();
            if parts.len() >= 4 {
                let key = parts[0].to_string();
                if parts[3].contains("rx") {
                    rx.insert(key, value);
                } else if parts[3].contains("tx") {
                    tx.insert(key, value);
                }
            }
        }
    }

    let connections_by_server = convert_connections(connections_by_type.clone());
    let mbps_by_server = convert_bps_to_mbps(rx.clone(), tx.clone());

    debug!("Connections {:?}", connections_by_server);
    debug!("Mbps {:?}", mbps_by_server);

    let response = StatusResponse {
        connections: connections_by_server,
        mbps: mbps_by_server,
    };

    HttpResponse::Ok().json(response)
}

#[get("/status/clickhouse")]
async fn status_ch(ch_client: Data<Arc<Client>>) -> impl Responder {
    let query = "SELECT toUInt32(1)";
    match ch_client.query(query).fetch_all::<u32>().await {
        Ok(_) => HttpResponse::Ok().body("Ok"),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {:?}", e)),
    }
}
