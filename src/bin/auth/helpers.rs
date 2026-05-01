use serde::Deserialize;

use fcore::{
    http::response::{Instance, InstanceWithId, ResponseMessage, SubscriptionResponse},
    Code, Env, Error, Key, Result, Subscription,
};

use super::http::HttpClient;

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
) -> Result<Key> {
    let url = format!("{}/key/validate?key={}", api_address, key);
    tracing::debug!("URL = {}", url);

    let res = auth_headers(http.get(url), api_token).send().await?;
    let status = res.status();
    let text = res.text().await?;

    if status.is_success() {
        let parsed: ResponseMessage<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

        tracing::debug!("Response validate_key {} {}", parsed.status, parsed.message);

        match parsed.response.instance {
            Instance::Key(key) => Ok(key),
            _ => Err(Error::Custom("Unexpected instance type".into())),
        }
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        Err(Error::Custom(
            err.message.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }
}

pub async fn get_subscription(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    subscription_id: &uuid::Uuid,
) -> Result<SubscriptionResponse> {
    let url = format!("{}/subscription/{}", api_address, subscription_id);
    tracing::debug!("URL = {}", url);

    let res = auth_headers(http.get(url), api_token).send().await?;
    let status = res.status();
    let text = res.text().await?;

    if status.is_success() {
        let parsed: SubscriptionResponse = serde_json::from_str(&text)?;

        tracing::debug!(
            "Response for GET subscription/{} {:?}",
            subscription_id,
            parsed
        );

        Ok(parsed)
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        Err(Error::Custom(
            err.message.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }
}

pub async fn activate_key(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    key: &Code,
    sub_id: &uuid::Uuid,
) -> Result<Key> {
    let url = format!("{}/key/activate?", api_address);
    tracing::debug!("URL = {}", url);

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
        let parsed: ResponseMessage<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

        tracing::debug!("Response activate_key {} {}", parsed.status, parsed.message);

        match parsed.response.instance {
            Instance::Key(key) => Ok(key),
            _ => Err(Error::Custom("Unexpected instance type".into())),
        }
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        Err(Error::Custom(
            err.message.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }
}

pub async fn create_subscription(
    http: &HttpClient,
    api_address: &str,
    api_token: &str,
    days: i64,
    referred_by: &str,
) -> Result<Subscription> {
    let url = format!("{}/subscription", api_address);
    tracing::debug!("URL = {}", url);

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
        let parsed: ResponseMessage<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

        match parsed.response.instance {
            Instance::Subscription(sub) => Ok(sub),
            _ => Err(Error::Custom("Unexpected instance type".into())),
        }
    } else {
        #[derive(Deserialize)]
        struct ErrResp {
            message: Option<String>,
        }
        let err: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp { message: None });
        Err(Error::Custom(
            err.message.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }
}

pub async fn create_connection(
    http: &HttpClient,
    env: &Env,
    proto: &str,
    sub_id: &uuid::Uuid,
    api_address: &str,
    api_token: &str,
) -> Result<uuid::Uuid> {
    tracing::debug!("POST /connection {}", env);

    let res = auth_headers(
        http.post(format!("{}/connection", api_address))
            .json(&serde_json::json!({
                "env": env.to_string(),
                "proto": proto,
                "subscription_id": sub_id
            })),
        api_token,
    )
    .send()
    .await?;

    let status = res.status();
    let text = res.text().await?;

    tracing::debug!("Connection resp: {}", text);

    if text.is_empty() {
        Error::Custom(format!("empty connection response, status = {}", status));
    }

    let parsed: ResponseMessage<InstanceWithId<Instance>> = serde_json::from_str(&text)?;

    tracing::debug!(
        "Response create_connection {} {}",
        parsed.status,
        parsed.message
    );

    Ok(parsed.response.id)
}
