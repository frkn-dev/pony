use std::sync::Arc;

use pony::metrics::storage::MetricStorage;

pub async fn handle_ws_client(
    socket: warp::ws::WebSocket,
    node_id: uuid::Uuid,
    metric: String,
    storage: Arc<MetricStorage>,
) {
    use futures::{SinkExt, StreamExt};
    let (mut ws_tx, _) = socket.split();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(1000));

    loop {
        ticker.tick().await;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let ten_min_ago_ms = now_ms - (10 * 60 * 1000);

        let points = storage.get_range(&node_id, &metric, ten_min_ago_ms, now_ms);

        if !points.is_empty() {
            let chart_points: Vec<serde_json::Value> = points
                .into_iter()
                .map(|p| serde_json::json!({ "x": p.timestamp, "y": p.value }))
                .collect();

            if let Ok(msg) = serde_json::to_string(&chart_points) {
                if let Err(e) = ws_tx.send(warp::ws::Message::text(msg)).await {
                    log::error!("WS send error: {}", e);
                    break;
                }
            }
        }
    }
}
