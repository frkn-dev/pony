use reqwest::Client;
use serde_json::Value;
use std::error::Error;

pub struct Locator {
    pub ip: String,
    pub latitude: String,
    pub longitude: String,
    pub city: String,
    pub region: String,
    pub country: String,
    pub timezone: String,
    pub location: String,
}

pub async fn find(ip: &str) -> Result<Locator, Box<dyn Error>> {
    let uri = format!("http://ip-api.com/json/{}", ip);
    let client = Client::new();
    let local_data_response = client.get(&uri).send().await?;

    let local_data: String = local_data_response.text().await?;
    let local_body: Value = serde_json::from_str(&local_data)?;

    let result = Locator {
        ip: local_body["query"].as_str().unwrap_or("").to_string(),
        latitude: local_body["lat"].as_str().unwrap_or("").to_string(),
        longitude: local_body["lon"].as_str().unwrap_or("").to_string(),
        city: local_body["city"].as_str().unwrap_or("").to_string(),
        region: local_body["regionName"].as_str().unwrap_or("").to_string(),
        country: local_body["country"].as_str().unwrap_or("").to_string(),
        timezone: local_body["timezone"].as_str().unwrap_or("").to_string(),
        location: format!(
            "{:?}, {:?}, {:?}",
            local_body["city"].as_str().unwrap_or(""),
            local_body["regionName"].as_str().unwrap_or(""),
            local_body["country"].as_str().unwrap_or("")
        ),
    };

    Ok(result)
}
