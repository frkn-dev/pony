use std::fmt;
use std::{net::Ipv4Addr, sync::Arc};

use log::debug;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use uuid::Uuid;
use warp::reject;
use warp::Filter;

use super::yookassa::{Amount, Metadata};
use crate::postgres::subscription::*;
use crate::postgres::DbContext;

#[derive(Deserialize, Debug)]
struct WebhookPayload {
    id: String,
    status: String,
    paid: bool,
    amount: Amount,
    metadata: Option<Metadata>,
}

#[derive(Debug)]
struct JsonError {
    err: String,
}
impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JsonError: {}", self.err)
    }
}

impl reject::Reject for JsonError {}

#[derive(Deserialize, Serialize)]
struct ResponseMessage<T> {
    status: u16,
    message: T,
}

async fn handle_webhook(
    payload: WebhookPayload,
    pg: Arc<Mutex<PgClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Webhook received: {:?}", payload);

    let db = DbContext::new(pg);

    if payload.status == "succeeded" && payload.paid {
        if let Some(metadata) = payload.metadata {
            let user_id = Uuid::parse_str(&metadata.user_id).map_err(|e| {
                log::error!("UUID parse error: {:?}", e);
                warp::reject::custom(JsonError {
                    err: "UUID parse error".to_string(),
                })
            })?;

            let amount = payload.amount.value.parse::<f64>().map_err(|e| {
                log::error!("Amount parse error: {:?}", e);
                warp::reject::custom(JsonError {
                    err: "Amount parse error".to_string(),
                })
            })?;

            let has_active = db
                .subscription()
                .has_active_subscription(user_id)
                .await
                .map_err(|e| {
                    log::error!("DB query error: {:?}", e);
                    warp::reject::custom(JsonError {
                        err: "DB query error".to_string(),
                    })
                })?;

            let existing_expired_at = db.subscription().subscription_exist(user_id).await;

            let subscription = SubscriptionRow {
                id: Uuid::new_v4(),
                user_id,
                payment_id: payload.id,
                amount,
                currency: payload.amount.currency,
                created_at: chrono::Utc::now().naive_utc(),
                expired_at: chrono::Utc::now().naive_utc(),
            };

            if has_active {
                db.subscription()
                    .add_subscription(subscription, existing_expired_at)
                    .await
                    .map_err(|e| {
                        log::error!("Error extending subscription: {:?}", e);
                        warp::reject::custom(JsonError {
                            err: "DB extend subscription error".to_string(),
                        })
                    })?;
            } else {
                db.subscription()
                    .insert_subscription(subscription)
                    .await
                    .map_err(|e| {
                        log::error!("Error inserting subscription: {:?}", e);
                        warp::reject::custom(JsonError {
                            err: "DB insert subscription error".to_string(),
                        })
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

pub async fn run_webhook_server(client: Arc<Mutex<PgClient>>, listen: Ipv4Addr, port: u16) {
    let webhook = warp::post()
        .and(warp::path("webhook"))
        .and(warp::body::json())
        .and(with_pg_client(client))
        .and_then(handle_webhook);

    debug!("Running webhook {}:{}", listen, port);

    warp::serve(webhook).run((listen, port)).await;
}

fn with_pg_client(
    client: Arc<Mutex<PgClient>>,
) -> impl Filter<Extract = (Arc<Mutex<PgClient>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || client.clone())
}
