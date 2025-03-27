use log::debug;
use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, sync::Arc};
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;
use warp::reject;
use warp::Filter;

use super::yookassa::{Amount, Metadata};
use crate::postgres::subscription::*;

#[derive(Deserialize, Debug)]
struct WebhookPayload {
    id: String,
    status: String,
    paid: bool,
    amount: Amount,
    metadata: Option<Metadata>,
}

#[derive(Debug)]
struct JsonError(String);

impl reject::Reject for JsonError {}

#[derive(Deserialize, Serialize)]
struct ResponseMessage<T> {
    status: u16,
    message: T,
}

async fn handle_webhook(
    payload: WebhookPayload,
    client: Arc<Mutex<Client>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Webhook received: {:?}", payload);

    if payload.status == "succeeded" && payload.paid {
        if let Some(metadata) = payload.metadata {
            let user_id = Uuid::parse_str(&metadata.user_id).map_err(|e| {
                log::error!("UUID parse error: {:?}", e);
                warp::reject::custom(JsonError("UUID parse error".to_string()))
            })?;

            let amount = payload.amount.value.parse::<f64>().map_err(|e| {
                log::error!("Amount parse error: {:?}", e);
                warp::reject::custom(JsonError("Amount parse error".to_string()))
            })?;

            let has_active = has_active_subscription(Arc::clone(&client), user_id)
                .await
                .map_err(|e| {
                    log::error!("DB query error: {:?}", e);
                    warp::reject::custom(JsonError("DB query error".to_string()))
                })?;

            let existing_expired_at = subscription_exist(Arc::clone(&client), user_id).await;

            let subscription = SubscriptionRow {
                id: Uuid::new_v4(),
                user_id,
                payment_id: payload.id,
                amount,
                currency: payload.amount.currency,
                created_at: chrono::Utc::now().naive_utc(),
                expired_at: chrono::Utc::now().naive_utc(), // placeholder
            };

            if has_active {
                add_subscription(client, subscription, existing_expired_at)
                    .await
                    .map_err(|e| {
                        log::error!("Error extending subscription: {:?}", e);
                        warp::reject::custom(JsonError("DB extend subscription error".to_string()))
                    })?;
            } else {
                insert_subscription(client, subscription)
                    .await
                    .map_err(|e| {
                        log::error!("Error inserting subscription: {:?}", e);
                        warp::reject::custom(JsonError("DB insert subscription error".to_string()))
                    })?;
            }
        }
    }

    let response = ResponseMessage {
        status: 200,
        message: "Subscription Created or Extended".to_string(),
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        warp::http::StatusCode::OK,
    ))
}

pub async fn run_webhook_server(client: Arc<Mutex<Client>>, listen: Ipv4Addr, port: u16) {
    let webhook = warp::post()
        .and(warp::path("webhook"))
        .and(warp::body::json())
        .and(with_pg_client(client))
        .and_then(handle_webhook);

    debug!("Running webhook {}:{}", listen, port);

    warp::serve(webhook).run((listen, port)).await;
}

fn with_pg_client(
    client: Arc<Mutex<Client>>,
) -> impl Filter<Extract = (Arc<Mutex<Client>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || client.clone())
}
