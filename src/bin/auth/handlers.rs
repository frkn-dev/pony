use std::sync::Arc;
use tokio::sync::RwLock;

use fcore::{
    http::{helpers as http, response::Instance},
    ApiAccessConfig, ConnectionBaseOperations, ConnectionStorageBaseOperations, Connections, Env,
};

#[cfg(feature = "email")]
use super::email::EmailStore;
use super::helpers::{activate_key, validate_key};
use super::helpers::{create_connection, create_subscription, get_subscription};
use super::http::HttpClient;
use super::request;
use super::response;
use super::service::DEFAULT_DAYS;
use super::service::PROTOS;

pub async fn activate_key_handler(
    req: request::ActivateKey,
    http: HttpClient,
    api: ApiAccessConfig,
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = match validate_key(&http, &api.endpoint, &api.token, &req.code).await {
        Ok(k) => k,
        Err(e) => {
            tracing::error!("Key code is not valid: {}", e);
            return Ok(http::bad_request(&format!("Failed: {}. ", e)));
        }
    };

    if key.activated {
        return Ok(http::bad_request("Failed: Key is already activated. "));
    }

    let subscription_id = if let Some(subscription_id) = req.subscription_id {
        subscription_id
    } else {
        let referred_by = "WEB";
        let sub =
            match create_subscription(&http, &api.endpoint, &api.token, DEFAULT_DAYS, referred_by)
                .await
            {
                Ok(s) => s,
                Err(e) => return Ok(http::internal_error(&format!("Creation failed: {}", e))),
            };

        if let Err(e) = setup_connections(&http, &api, &sub.id).await {
            tracing::error!("{}", e);
            return Ok(http::internal_error("Failed to establish connections."));
        }

        sub.id
    };

    let sub = match get_subscription(&http, &api.endpoint, &api.token, &subscription_id).await {
        Ok(s) => s,
        Err(e) => return Ok(http::not_found(&format!("Subscription not found: {}", e))),
    };

    let activated_key =
        match activate_key(&http, &api.endpoint, &api.token, &req.code, &sub.id).await {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("Code activation failed: {}", e);
                return Ok(http::bad_request(&format!("Activation failed: {}", e)));
            }
        };

    let mut updated_sub = sub.clone();
    let now = chrono::Utc::now();
    let base_date = if updated_sub.expires > now {
        updated_sub.expires
    } else {
        now
    };
    updated_sub.expires = base_date + chrono::Duration::days(activated_key.days as i64);

    Ok(http::success_response(
        "Key code activated.".to_string(),
        Some(activated_key.id),
        Instance::SubscriptionResponse(updated_sub),
    ))
}

#[cfg(feature = "email")]
pub async fn trial_handler(
    req: request::Trial,
    store: EmailStore,
    http: HttpClient,
    api: ApiAccessConfig,
) -> Result<impl warp::Reply, warp::Rejection> {
    if store.check_email_hmac(&req.email).await {
        return Ok(http::bad_request("Trial already requested"));
    }

    let ref_by = req.referred_by.clone().unwrap_or_else(|| "WEB".to_string());

    let sub =
        match create_subscription(&http, &api.endpoint, &api.token, DEFAULT_DAYS, &ref_by).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(http::internal_error(&format!("Subscription failed: {}", e)));
            }
        };

    if let Err(e) = setup_connections(&http, &api, &sub.id).await {
        tracing::error!("{}", e);
        return Ok(http::internal_error(
            "Failed to establish trial connections.",
        ));
    }

    let now = chrono::Utc::now();

    if let Err(e) = store
        .save_trial_hmac(&req.email, &sub.id, &now, &ref_by)
        .await
    {
        tracing::error!("HMAC save error: {}", e);
        return Ok(http::internal_error("Failed to record trial."));
    }

    store.send_email_background(req.email.clone(), sub.id).await;

    Ok(http::success_response(
        "Trial activated. Check email".into(),
        Some(sub.id),
        Instance::None,
    ))
}

pub async fn tg_trial_handler(
    req: request::TgTrial,
    http: HttpClient,
    api: ApiAccessConfig,
) -> Result<impl warp::Reply, warp::Rejection> {
    let referred_by = req.referred_by.unwrap_or_else(|| "TG".to_string());

    // 1. Create subscription
    let sub =
        match create_subscription(&http, &api.endpoint, &api.token, DEFAULT_DAYS, &referred_by)
            .await
        {
            Ok(s) => s,
            Err(e) => return Ok(http::internal_error(&format!("Subscription failed: {}", e))),
        };

    // 2. Create connections
    if let Err(e) = setup_connections(&http, &api, &sub.id).await {
        tracing::error!("{}", e);
        return Ok(http::internal_error(
            "Failed to establish trial connections.",
        ));
    }

    Ok(http::success_response(
        "Trial activated".into(),
        Some(sub.id),
        Instance::Subscription(sub),
    ))
}

pub async fn auth_handler<C>(
    req: request::Auth,
    memory: Arc<RwLock<Connections<C>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    C: ConnectionBaseOperations + Sync + Send + Clone + 'static + std::fmt::Display,
{
    tracing::debug!("Auth req {} {} {}", req.auth, req.addr, req.tx);
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

async fn setup_connections(
    http: &HttpClient,
    api: &ApiAccessConfig,
    sub_id: &uuid::Uuid,
) -> Result<(), String> {
    let envs = [Env::Production, Env::Dev, Env::Ru, Env::Wl];
    let futures = envs.iter().flat_map(|env| {
        PROTOS.iter().map(move |proto| {
            create_connection(http, env, proto, sub_id, &api.endpoint, &api.token)
        })
    });

    let results = futures::future::join_all(futures).await;
    if results.iter().any(|r| r.is_err()) {
        return Err(format!(
            "Failed to establish connections for sub {}",
            sub_id
        ));
    }
    Ok(())
}
