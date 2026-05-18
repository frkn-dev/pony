use crate::http::StatusCode;
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use std::collections::BTreeMap;
use std::sync::Arc;

use fcore::MetricStorage;

/// Debug endpoint
pub async fn debug_metrics_handler(
    metrics: Arc<MetricStorage>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut debug_info = serde_json::json!({
        "total_nodes": metrics.inner.len(),
        "nodes": []
    });

    let nodes_array = debug_info.as_object_mut().unwrap();
    let mut nodes_list = Vec::new();

    for node_ref in metrics.inner.iter() {
        let node_id = node_ref.key();
        let node_metrics = node_ref.value();

        let mut metrics_list = Vec::new();

        for metric_entry in node_metrics.iter() {
            let series_hash = metric_entry.key();
            let points = metric_entry.value();

            let (name, tags) = match metrics.metadata.get(series_hash) {
                Some(meta) => (meta.0.clone(), meta.1.clone()),
                None => ("unknown".to_string(), BTreeMap::new()),
            };

            metrics_list.push(serde_json::json!({
                "hash": series_hash.to_string(),
                "name": name,
                "tags": tags,
                "points_count": points.len(),
                "last_value": points.back().map(|p| p.value),
                "last_timestamp": points.back().map(|p| p.timestamp),
            }));
        }

        nodes_list.push(serde_json::json!({
            "node_id": node_id,
            "metrics_count": metrics_list.len(),
            "metrics": metrics_list,
        }));
    }

    nodes_array.insert("nodes".to_string(), serde_json::Value::Array(nodes_list));

    Ok(warp::reply::with_status(
        warp::reply::json(&debug_info),
        StatusCode::OK,
    ))
}

pub async fn handle_ws_client(
    socket: warp::ws::WebSocket,
    node_id: uuid::Uuid,
    series_hash: u64,
    storage: Arc<MetricStorage>,
) {
    let (mut ws_tx, _) = socket.split();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(1000));

    loop {
        ticker.tick().await;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let ten_min_ago_ms = now_ms - (10 * 60 * 1000);

        let points = storage.get_range(&node_id, series_hash, ten_min_ago_ms, now_ms);

        if !points.is_empty() {
            let msg = serde_json::json!({
                "type": "update",
                "node_id": node_id,
                "hash": series_hash,
                "data": points.iter().map(|p| (p.timestamp, p.value)).collect::<Vec<_>>()
            });

            if ws_tx
                .send(warp::ws::Message::text(msg.to_string()))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

pub async fn handle_aggregated_ws(
    socket: warp::ws::WebSocket,
    tag_key: String,
    tag_value: String,
    metric_name: String,
    storage: Arc<MetricStorage>,
) {
    let (mut ws_tx, _) = socket.split();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(1000));

    loop {
        ticker.tick().await;

        let now = Utc::now().timestamp_millis();
        let aggregated_data =
            storage.get_aggregated_range(&tag_key, &tag_value, &metric_name, now - 600_000, now);

        if !aggregated_data.is_empty() {
            let response = aggregated_data
                .iter()
                .map(|(id, points)| {
                    (
                        id,
                        points
                            .iter()
                            .map(|p| (p.timestamp, p.value))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();

            let msg = serde_json::json!({
                "type": "aggregated_update",
                "tag": format!("{}:{}", tag_key, tag_value),
                "metric": metric_name,
                "data": response
            });

            if ws_tx
                .send(warp::ws::Message::text(msg.to_string()))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}
