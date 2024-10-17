use reqwest::Client;
use serde_json::Value;
use std::error::Error;

pub struct Locator {
    pub country: String,
}

pub async fn find(ip: &str) -> Result<Locator, Box<dyn Error>> {
    let uri = format!("http://ip-api.com/json/{}", ip);
    let client = Client::new();
    let local_data_response = client.get(&uri).send().await?;

    let local_data: String = local_data_response.text().await?;
    let local_body: Value = serde_json::from_str(&local_data)?;

    let result = Locator {
        country: local_body["country"].as_str().unwrap_or("").to_string(),
    };

    Ok(result)
}
