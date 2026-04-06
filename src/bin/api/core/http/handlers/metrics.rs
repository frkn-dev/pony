use std::sync::Arc;

use pony::metrics::storage::MetricStorage;

pub async fn get_heartbeat_handler(
    node_id: String,
    storage: Arc<MetricStorage>,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let full_key = format!("{}.heartbeat", node_id);

    let now = chrono::Utc::now().timestamp();
    let from = now - 300; // за последние 5 минут

    let points = storage.get_range(&full_key, from, now).unwrap_or_default();

    // Форматируем для Chart.js (x: ms, y: value)
    let chart_data: Vec<serde_json::Value> = points
        .into_iter()
        .map(|p| serde_json::json!({ "x": p.timestamp * 1000, "y": p.value }))
        .collect();

    Ok(warp::reply::json(&chart_data))
}
