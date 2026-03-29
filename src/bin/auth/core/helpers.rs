use super::response;
use super::Env;
use super::HttpClient;
use pony::http::helpers::{Instance, InstanceWithId};
use pony::memory::key::Code;
use pony::Subscription;

use pony::memory::key::Key;
use serde::Deserialize;

fn auth_headers(req: reqwest::RequestBuilder, api_token: &str) -> reqwest::RequestBuilder {
    req.header("Authorization", format!("Bearer {}", api_token))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
}

pub async fn validate_key(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    key: &Code,
) -> anyhow::Result<Key> {
    let url = format!("{}/key/validate?key={}", api_address, key);
    log::debug!("URL = {}", url);

    let res = auth_headers(http.get(url), api_token).send().await?;
    let status = res.status();
    let text = res.text().await?;

    if status.is_success() {
        let parsed: response::Api<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

        log::debug!("Response validate_key {} {}", parsed.status, parsed.message);

        match parsed.response.instance {
            Instance::Key(key) => Ok(key),
            _ => anyhow::bail!("Unexpected instance type"),
        }
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        anyhow::bail!(err.message.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

pub async fn activate_key(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    key: &Code,
    sub_id: &uuid::Uuid,
) -> anyhow::Result<Key> {
    let url = format!("{}/key/activate?", api_address);
    log::debug!("URL = {}", url);

    let res = auth_headers(
        http.post(url).json(&serde_json::json!({
            "subscription_id": sub_id,
            "code": key,
        })),
        api_token,
    )
    .send()
    .await?;
    let status = res.status();
    let text = res.text().await?;

    if status.is_success() {
        let parsed: response::Api<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

        log::debug!("Response activate_key {} {}", parsed.status, parsed.message);

        match parsed.response.instance {
            Instance::Key(key) => Ok(key),
            _ => anyhow::bail!("Unexpected instance type"),
        }
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        anyhow::bail!(err.message.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

pub async fn create_subscription(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    days: i64,
    referred_by: &str,
) -> anyhow::Result<Subscription> {
    let url = format!("{}/subscription", api_address);
    log::debug!("URL = {}", url);

    let res = auth_headers(
        http.post(url).json(&serde_json::json!({
            "days": days,
            "referred_by": referred_by.to_string()
        })),
        api_token,
    )
    .send()
    .await?;

    let status = res.status();
    let text = res.text().await?;

    if status.is_success() {
        let parsed: response::Api<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

        match parsed.response.instance {
            Instance::Subscription(sub) => Ok(sub),
            _ => anyhow::bail!("Unexpected instance type"),
        }
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        anyhow::bail!(err.message.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

pub async fn create_connection(
    http: &HttpClient,
    env: &Env,
    proto: &str,
    sub_id: &uuid::Uuid,
    token: &Option<uuid::Uuid>,
    api_address: &str,
    api_token: &str,
) -> anyhow::Result<uuid::Uuid> {
    let res = if let Some(token) = token {
        auth_headers(
            http.post(format!("{}/connection", api_address))
                .json(&serde_json::json!({
                    "env": env.to_string(),
                    "proto": proto,
                    "subscription_id": sub_id,
                    "token": token,
                })),
            api_token,
        )
        .send()
        .await?
    } else {
        auth_headers(
            http.post(format!("{}/connection", api_address))
                .json(&serde_json::json!({
                    "env": env.to_string(),
                    "proto": proto,
                    "subscription_id": sub_id
                })),
            api_token,
        )
        .send()
        .await?
    };

    let status = res.status();
    let text = res.text().await?;

    log::debug!("Connection resp: {}", text);

    if text.is_empty() {
        anyhow::bail!("empty connection response, status = {}", status);
    }

    let parsed: response::Api<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

    log::debug!(
        "Response create_connection {} {}",
        parsed.status,
        parsed.message
    );

    Ok(parsed.response.id)
}
