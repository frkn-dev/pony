use log::debug;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;

use crate::{insert_user, user_exist, UserRow};

pub async fn register(
    username: &str,
    client: Arc<Mutex<PgClient>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(_user) = user_exist(client.clone(), username.to_string()).await {
        Err("User already exist".into())
    } else {
        log::info!("Registering user");
        let user = UserRow::new(username);
        insert_user(client, user).await
    }
}

pub async fn create_vpn_user(
    username: String,
    endpoint: String,
    token: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut endpoint = Url::parse(&endpoint)?;
    endpoint
        .path_segments_mut()
        .map_err(|_| "Invalid API endpoint")?
        .push("user");
    let endpoint = endpoint.to_string();

    debug!("ENDPOINT: {}", endpoint);

    let user = UserRow::new(&username);
    let user_req = user.as_create_user_request();
    let json = serde_json::to_string_pretty(&user_req).unwrap();

    println!("JSON {}", json);

    let res = HttpClient::new()
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .json(&user_req)
        .send()
        .await?;

    if res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED {
        return Ok(());
    } else {
        return Err(format!("Req error: {} {:?}", res.status(), res).into());
    }
}
