use actix_web::web::{Data, Path};
use actix_web::{get, HttpResponse, Responder};
use clickhouse::Client;
use log::debug;
use log::error;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::clickhouse::fetch_metrics_value;

#[derive(Deserialize)]
struct Params {
    env: String,
    cluster: String,
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
    let metrics_connections =
        fetch_metrics_value(&ch_client, &req.env, &req.cluster, "connections%").await;
    let metrics_value_connections = match metrics_connections {
        Ok(metrics) => metrics
            .into_iter()
            .map(|m| (m.metric.clone(), m.value.clone()))
            .collect(),
        Err(err) => {
            error!("Couldn't get metrics {}", err);
            vec![]
        }
    };

    let mut metrics_map_result: HashMap<String, f64> = HashMap::new();
    let mut prefix_sums_connections: HashMap<String, f64> = HashMap::new();

    for (metric, value) in metrics_value_connections {
        if let Some(stripped_metric) = metric.strip_prefix(format!("{}.", req.cluster).as_str()) {
            let parts: Vec<&str> = stripped_metric.split(".connections.").collect();
            if parts.len() == 2 {
                let key = format!("connections.{}.{}", parts[0], parts[1]);
                metrics_map_result.insert(key, value);

                let prefix = parts[0].to_string();
                *prefix_sums_connections.entry(prefix).or_insert(0.0) += value;
            }
        }
    }

    let metrics_bps = fetch_metrics_value(&ch_client, &req.env, &req.cluster, "%bps").await;
    let metrics_value_bps = match metrics_bps {
        Ok(metrics) => metrics
            .into_iter()
            .map(|m| (m.metric.clone(), m.value.clone()))
            .collect(),
        Err(err) => {
            error!("Couldn't get metrics {}", err);
            vec![]
        }
    };

    for (metric, value) in metrics_value_bps {
        if let Some(stripped_metric) = metric.strip_prefix("dev.") {
            let parts: Vec<&str> = stripped_metric.split(".").collect();
            let key = format!("bps.{}.{}.{}", parts[0], parts[2], parts[3]);
            metrics_map_result.insert(key, value);
        }
    }

    for (prefix, sum) in prefix_sums_connections {
        metrics_map_result.insert(format!("connections.{}.total", prefix), sum);
    }

    HttpResponse::Ok().json(metrics_map_result)
}

#[get("/status/clickhouse")]
async fn status_ch(ch_client: Data<Arc<Client>>) -> impl Responder {
    let query = "SELECT toUInt32(1)";
    debug!("Req /status/clickhouse");
    match ch_client.query(query).fetch_all::<u32>().await {
        Ok(_) => HttpResponse::Ok().body("Ok"),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {:?}", e)),
    }
}
