use actix_web::web::{Data, Path};
use actix_web::{get, HttpResponse, Responder};
use clickhouse::Client;
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

#[derive(Serialize)]
struct Connection {
    #[serde(flatten)]
    connection: HashMap<String, f64>,
}

#[derive(Serialize)]
struct ConnectionsByType {
    vless: Vec<Connection>,
    vmess: Vec<Connection>,
    ss: Vec<Connection>,
    wg: Vec<Connection>,
}

#[derive(Serialize)]
struct Bps {
    rx: HashMap<String, f64>,
    tx: HashMap<String, f64>,
}

#[derive(Serialize)]
struct StatusResponse {
    connections: ConnectionsByType,
    bps: Bps,
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

    let response = StatusResponse {
        connections: connections_by_type,
        bps: Bps { rx, tx },
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
