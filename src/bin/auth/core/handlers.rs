use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::helpers::{activate_key, validate_key};

use super::helpers::{create_connection, create_subscription};
use super::request;
use super::response;
use super::EmailStore;
use super::HttpClient;

use super::Env;
use super::DEFAULT_DAYS;
use super::PROTOS;
use pony::config::settings::ApiAccessConfig;
use pony::http::helpers as http;
use pony::http::response::Instance;
use pony::memory::connection::Connections;
use pony::ConnectionBaseOp;
use pony::ConnectionStorageBaseOp;

pub async fn activate_key_handler(
    req: request::Key,
    store: EmailStore,
    http: HttpClient,
    api: ApiAccessConfig,
) -> Result<impl warp::Reply, warp::Rejection> {
    use futures::future::join_all;

    /* ================= Key Code Validation ================= */

    let key = match validate_key(&http, &api.endpoint, &api.token, &req.code).await {
        Ok(k) => k,
        Err(e) => {
            log::error!("Key code is not valid: {}", e);
            return Ok(http::bad_request(&format!("Failed: {}. ", e)));
        }
    };

    if key.activated {
        return Ok(http::bad_request("Failed: Key is already activated. "));
    }

    /* ================= CREATE SUBSCRIPTION ================= */

    let referred_by = "FRKN.ORG";

    let sub = match create_subscription(&http, &api.endpoint, &api.token, DEFAULT_DAYS, referred_by)
        .await
    {
        Ok(sub) => sub,
        Err(e) => {
            log::error!("Subscription creation failed: {}", e);
            return Ok(http::internal_error(&format!(
                "Failed: {}. Please try again later or contact support.",
                e
            )));
        }
    };

    /* ================== ACTIVATE KEY  ======================= */

    let key = match activate_key(&http, &api.endpoint, &api.token, &req.code, &sub.id).await {
        Ok(k) => k,
        Err(e) => {
            log::error!("Code activation is failed: {}", e);
            return Ok(http::bad_request(&format!("Failed: {}. ", e)));
        }
    };

    /* Update subscriptions days  */

    let mut updated_sub = sub.clone();
    if let Some(expires) = updated_sub.expires_at {
        updated_sub.expires_at = Some(expires + chrono::Duration::days(key.days as i64));
    }

    /* ================= CREATE CONNECTIONS ================= */

    let envs = [Env::Dev, Env::Ru, Env::Wl];

    let futures = envs.iter().flat_map(|env| {
        PROTOS.iter().map({
            let api_token = api.token.clone();
            let api_endpoint = api.endpoint.clone();
            let http = http.clone();
            move |proto| {
                let http = http.clone();
                let api_endpoint = api_endpoint.clone();
                let api_token = api_token.clone();

                async move {
                    let token = if proto == &"Hysteria2" {
                        Some(uuid::Uuid::new_v4())
                    } else {
                        None
                    };

                    create_connection(
                        &http,
                        env,
                        proto,
                        &sub.id,
                        &token,
                        &api_endpoint,
                        &api_token,
                    )
                    .await
                }
            }
        })
    });

    let results = join_all(futures).await;

    if results.iter().any(|r| r.is_err()) {
        log::error!("One or more connections failed for subscription {}", sub.id);
        return Ok(http::internal_error(
            "Failed to establish connections. Please try again later or contact support.",
        ));
    }

    /* ================= SEND EMAIL + SAVE ================= */

    if let Some(email) = req.email {
        if let Err(e) = store.send_email(&email, &sub.id).await {
            log::error!("email error: {}", e);
            return Ok(http::internal_error(
                "Failed to send email. Please try again later or contact support.",
            ));
        }
    }

    Ok(http::success_response(
        "Key code activated.".to_string(),
        Some(key.id),
        Instance::Subscription(updated_sub),
    ))
}

pub async fn trial_handler(
    req: request::Trial,
    store: EmailStore,
    http: HttpClient,
    api: ApiAccessConfig,
) -> Result<impl warp::Reply, warp::Rejection> {
    use chrono::Utc;
    use futures::future::join_all;

    /* ================= ATOMIC TRIAL CHECK ================= */

    {
        let exists = store.check_email_hmac(&req.email).await;
        if exists {
            return Ok(http::bad_request("Trial already requested"));
        }
    }

    /* ================= CREATE SUBSCRIPTION ================= */

    let referred_by = req.referred_by.unwrap_or_else(|| "FRKN.ORG".to_string());

    let sub =
        match create_subscription(&http, &api.endpoint, &api.token, DEFAULT_DAYS, &referred_by)
            .await
        {
            Ok(sub) => sub,
            Err(e) => {
                log::error!("Subscription creation failed: {}", e);
                return Ok(http::internal_error(&format!(
                    "Failed: {}. Please try again later or contact support.",
                    e
                )));
            }
        };

    /* ================= CREATE CONNECTIONS ================= */

    let envs = [Env::Dev, Env::Ru, Env::Wl];

    let futures = envs.iter().flat_map(|env| {
        PROTOS.iter().map({
            let api_token = api.token.clone();
            let api_endpoint = api.endpoint.clone();
            let http = http.clone();
            move |proto| {
                let http = http.clone();
                let api_endpoint = api_endpoint.clone();
                let api_token = api_token.clone();

                async move {
                    let token = if proto == &"Hysteria2" {
                        Some(uuid::Uuid::new_v4())
                    } else {
                        None
                    };

                    create_connection(
                        &http,
                        env,
                        proto,
                        &sub.id,
                        &token,
                        &api_endpoint,
                        &api_token,
                    )
                    .await
                }
            }
        })
    });

    let results = join_all(futures).await;

    if results.iter().any(|r| r.is_err()) {
        log::error!("One or more connections failed for subsctiption {}", sub.id);
        return Ok(http::internal_error(
            "Failed to establish trial connections. Please try again later or contact support.",
        ));
    }

    /* ================= SEND EMAIL + SAVE ================= */

    let now = Utc::now();
    let email = req.email.clone();
    let ref_by = referred_by.clone();

    if let Err(e) = store.send_email(&email, &sub.id).await {
        log::error!("📧 email error: {}", e);
        return Ok(http::internal_error(
            "Failed to send confirmation email. Please try again later or contact support.",
        ));
    }

    if let Err(e) = store.save_trial_hmac(&email, &sub.id, &now, &ref_by).await {
        log::error!("Hamc email save error: {}", e);
        return Ok(http::internal_error(
            "Failed to record trial. Please contact support if the issue persists.",
        ));
    }

    Ok(http::success_response(
        "Trial activated. Check your email".to_string(),
        Some(sub.id),
        Instance::None,
    ))
}

pub async fn auth_handler<C>(
    req: request::Auth,
    memory: Arc<RwLock<Connections<C>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    C: ConnectionBaseOp + Sync + Send + Clone + 'static + std::fmt::Display,
{
    log::debug!("Auth req {} {} {}", req.auth, req.addr, req.tx);
    let mem = memory.read().await;
    if let Some(id) = mem.validate_token(&req.auth) {
        Ok(warp::reply::json(&response::Auth {
            ok: true,
            id: Some(id.to_string()),
        }))
    } else {
        Ok(warp::reply::json(&response::Auth {
            ok: false,
            id: None,
        }))
    }
}
