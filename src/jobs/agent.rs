use std::{error::Error, sync::Arc};

use log::{debug, error};
use reqwest::Url;
use reqwest::{Client, StatusCode};
use tokio::sync::Mutex;

use crate::http::handlers::ResponseMessage;
use crate::metrics::metrics::{collect_metrics, send_to_carbon, MetricType};
use crate::state::state::NodeStorage;
use crate::state::state::State;
use crate::state::state::UserStorage;
use crate::xray_op::client::{HandlerClient, StatsClient};
use crate::xray_op::{stats, stats::Prefix};

pub async fn collect_stats_job<T>(
    stats_client: Arc<Mutex<StatsClient>>,
    handler_client: Arc<Mutex<HandlerClient>>,
    state: Arc<Mutex<State<T>>>,
) -> Result<(), Box<dyn Error>>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let mut tasks = vec![];

    debug!("Running xray stat job");

    let user_ids = {
        let state = state.lock().await;
        state.users.keys().cloned().collect::<Vec<_>>()
    };

    for user_id in user_ids {
        let stats_client = Arc::clone(&stats_client);
        let state = state.clone();

        tasks.push(tokio::spawn(async move {
            if let Ok(user_stat) =
                stats::get_user_stats(stats_client, Prefix::UserPrefix(user_id)).await
            {
                let mut state = state.lock().await;
                if let Err(e) = state.update_user_downlink(user_id, user_stat.downlink) {
                    error!("Failed to update user downlink: {}", e);
                }
                if let Err(e) = state.update_user_uplink(user_id, user_stat.uplink) {
                    error!("Failed to update user uplink: {}", e);
                }
                if let Err(e) = state.update_user_online(user_id, user_stat.online) {
                    error!("Failed to update user online: {}", e);
                }
            } else {
                error!("Failed to fetch user stats for {}", user_id);
            }
        }));
    }

    if let Some(node) = {
        let state = state.lock().await;
        state.nodes.get_node()
    } {
        let node_tags = node.inbounds.keys().cloned().collect::<Vec<_>>();

        for tag in node_tags {
            let stats_client = stats_client.clone();
            let handler_client = handler_client.clone();
            let state = state.clone();
            let env = node.env.clone();

            tasks.push(tokio::spawn(async move {
                if let Ok(inbound_stat) = stats::get_inbound_stats(
                    stats_client,
                    handler_client,
                    Prefix::InboundPrefix(tag.clone()),
                )
                .await
                {
                    let mut state = state.lock().await;
                    if let Err(e) = state.nodes.update_node_downlink(
                        tag.clone(),
                        inbound_stat.downlink,
                        env.clone(),
                        node.uuid,
                    ) {
                        error!("Failed to update inbound downlink: {}", e);
                    }
                    if let Err(e) = state.nodes.update_node_uplink(
                        tag.clone(),
                        inbound_stat.uplink,
                        env.clone(),
                        node.uuid,
                    ) {
                        error!("Failed to update inbound uplink: {}", e);
                    }
                    if let Err(e) = state.nodes.update_node_user_count(
                        tag.clone(),
                        inbound_stat.user_count,
                        env.clone(),
                        node.uuid,
                    ) {
                        error!("Failed to update user_count: {}", e);
                    }
                }
            }));
        }
    }

    let results = futures::future::join_all(tasks).await;
    for result in results {
        if let Err(e) = result {
            error!("Task panicked: {:?}", e);
        }
    }

    Ok(())
}

pub async fn send_metrics_job<T>(
    state: Arc<Mutex<State<T>>>,
    carbon_address: String,
) -> Result<(), Box<dyn Error>>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    let metrics = collect_metrics::<T>(Arc::clone(&state)).await;

    for metric in metrics {
        match metric {
            MetricType::F32(m) => send_to_carbon(&m, &carbon_address).await?,
            MetricType::F64(m) => send_to_carbon(&m, &carbon_address).await?,
            MetricType::I64(m) => send_to_carbon(&m, &carbon_address).await?,
            MetricType::U64(m) => send_to_carbon(&m, &carbon_address).await?,
            MetricType::U8(m) => send_to_carbon(&m, &carbon_address).await?,
        }
    }
    Ok(())
}

pub async fn register_node<T>(
    state: Arc<Mutex<State<T>>>,
    endpoint: String,
    token: String,
) -> Result<(), Box<dyn Error>>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    let state = state.lock().await;

    let node = state
        .nodes
        .get_nodes(None)
        .and_then(|mut nodes| nodes.pop())
        .ok_or("No node available to register")?;

    let mut endpoint_url = Url::parse(&endpoint)?;
    endpoint_url
        .path_segments_mut()
        .map_err(|_| "Invalid API endpoint")?
        .push("node")
        .push("register");

    let endpoint_str = endpoint_url.to_string();
    debug!("ENDPOINT: {}", endpoint_str);

    match serde_json::to_string_pretty(&node) {
        Ok(json) => debug!("Serialized node for environment '{}': {}", node.env, json),
        Err(e) => error!("Error serializing node '{}': {}", node.hostname, e),
    }

    let res = Client::new()
        .post(&endpoint_str)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .json(&node)
        .send()
        .await?;

    let status = res.status();
    let body = res.text().await?;

    if status.is_success() || status == StatusCode::NOT_MODIFIED {
        if body.trim().is_empty() {
            debug!("Node is already registered");
            Ok(())
        } else {
            let parsed: ResponseMessage<String> = serde_json::from_str(&body)?;
            debug!("Node is already registered: {:?}", parsed);
            Ok(())
        }
    } else {
        error!("Registration failed: {} - {}", status, body);
        Err(format!("Registration failed: {} - {}", status, body).into())
    }
}
