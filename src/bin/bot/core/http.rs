use async_trait::async_trait;
use pony::http::ResponseMessage;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;

use pony::http::requests::UserConnQueryParam;
use pony::http::requests::UserRegQueryParam;
use pony::state::ConnStat;
use pony::{PonyError, Result};

use super::BotState;

pub enum RegisterStatus {
    Ok,
    AlreadyExist,
}

#[async_trait]
pub trait ApiRequests {
    async fn register_user(&self, username: &str) -> Result<RegisterStatus>;
    async fn get_user_vpn_connection(&self, username: &str) -> Result<Option<Vec<String>>>;
    async fn get_user_traffic_stat(
        &self,
        username: &str,
    ) -> Result<Option<Vec<(uuid::Uuid, ConnStat)>>>;
}

#[async_trait]
impl ApiRequests for BotState {
    async fn register_user(&self, username: &str) -> Result<RegisterStatus> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)?;
        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?
            .push("user")
            .push("register");

        let endpoint = endpoint.to_string();

        let user = UserRegQueryParam {
            username: username.to_string(),
        };

        let res = HttpClient::new()
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", &self.settings.api.token),
            )
            .json(&user)
            .send()
            .await?;

        if res.status().is_success() {
            return Ok(RegisterStatus::Ok);
        } else if res.status() == StatusCode::NOT_MODIFIED {
            return Ok(RegisterStatus::AlreadyExist);
        } else {
            return Err(PonyError::Custom(format!(
                "/user/register req error: {} {:?}",
                res.status(),
                res
            ))
            .into());
        }
    }

    async fn get_user_vpn_connection(&self, username: &str) -> Result<Option<Vec<String>>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?;

        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Cannot modify API endpoint path".to_string()))?
            .push("user")
            .push("connection");

        let query = UserConnQueryParam {
            username: username.to_string(),
            env: "all".to_string(),
            password: None,
            limit: 1024,
            trial: true,
        };

        let query_str = serde_urlencoded::to_string(&query)?;
        endpoint.set_query(Some(&query_str));

        let endpoint = endpoint.to_string();

        let res = HttpClient::new()
            .get(&endpoint)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.settings.api.token),
            )
            .send()
            .await?;

        log::debug!("result {:?} ", res);

        match res.status() {
            StatusCode::OK | StatusCode::NOT_MODIFIED => {
                let body = res.text().await?;
                let data: Vec<String> = serde_json::from_str(&body)?;
                Ok(Some(data))
            }
            StatusCode::ACCEPTED => Ok(None),
            _ => Err(PonyError::Custom(format!(
                "get_user_vpn_connection request error: {}",
                res.status()
            ))
            .into()),
        }
    }

    async fn get_user_traffic_stat(
        &self,
        username: &str,
    ) -> Result<Option<Vec<(uuid::Uuid, ConnStat)>>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?;

        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Cannot modify API endpoint path".to_string()))?
            .push("user")
            .push("stat");

        let query = UserRegQueryParam {
            username: username.to_string(),
        };

        let query_str = serde_urlencoded::to_string(&query)?;
        endpoint.set_query(Some(&query_str));

        let endpoint = endpoint.to_string();

        let res = HttpClient::new()
            .get(&endpoint)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.settings.api.token),
            )
            .send()
            .await?;

        log::debug!("result {:?} ", res);

        if res.status().is_success() {
            let body = res.text().await?;
            log::debug!("body: {}", body);
            let data: ResponseMessage<Vec<(uuid::Uuid, ConnStat)>> = serde_json::from_str(&body)?;

            Ok(Some(data.message))
        } else {
            return Err(PonyError::Custom(format!(
                "/user/stat req error: {} {:?}",
                res.status(),
                res
            ))
            .into());
        }
    }
}
