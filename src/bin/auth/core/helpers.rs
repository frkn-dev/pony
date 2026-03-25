use super::response;
use super::Env;
use super::HttpClient;
use pony::http::helpers::{Instance, InstanceWithId};

use serde::Deserialize;

fn auth_headers(req: reqwest::RequestBuilder, api_token: &str) -> reqwest::RequestBuilder {
    req.header("Authorization", format!("Bearer {}", api_token))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
}

pub async fn create_subscription(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    days: i64,
    referred_by: &str,
) -> anyhow::Result<uuid::Uuid> {
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

    log::debug!("STATUS = {}", status);
    log::debug!("BODY = {}", text);

    if status.is_success() {
        let parsed: response::Api<InstanceWithId<Instance>> = serde_json::from_str(&text)?;
        Ok(parsed.response.id)
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

    Ok(parsed.response.id)
}
