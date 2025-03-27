use log::debug;
use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, sync::Arc};
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;
use warp::reject;
use warp::Filter;

use super::yookassa::{Amount, Metadata};
use crate::postgres::subscription::{insert_subscription, SubscriptionRow};

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
    debug!("Webhook recieved: {:?}", payload);

    if payload.status == "succeeded" && payload.paid {
        if let Some(metadata) = payload.metadata {
            let subscription = SubscriptionRow {
                id: Uuid::new_v4(),
                user_id: Uuid::parse_str(&metadata.user_id).map_err(|e| {
                    eprintln!("Error parse UUID: {:?}", e);
                    warp::reject::custom(JsonError("UUID parse error".to_string()))
                })?,
                payment_id: payload.id,
                amount: payload.amount.value.parse::<f64>().unwrap_or(0.0),
                currency: payload.amount.currency,
                created_at: chrono::Utc::now().naive_utc(),
            };

            insert_subscription(client, subscription)
                .await
                .map_err(|e| {
                    eprintln!("Ошибка записи в БД: {:?}", e);
                    warp::reject::custom(JsonError("DB Write Error".to_string()))
                })?;
        }
    }

    let response = ResponseMessage {
        status: 200,
        message: "Subscription Created".to_string(),
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
