use chrono::Utc;
use futures::{SinkExt, StreamExt};
use std::collections::BTreeMap;
use std::sync::Arc;

use pony::MetricStorage;

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
