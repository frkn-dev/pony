use async_trait::async_trait;

use pony::http::requests::UserIdQueryParam;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;

use pony::http::requests::UserConnQueryParam;
use pony::http::requests::UserRegQueryParam;
use pony::http::ResponseMessage;
use pony::state::ConnStat;
use pony::state::User;
use pony::{PonyError, Result};

use super::BotState;

#[async_trait]
pub trait ApiRequests {
    async fn register_user(&self, username: &str) -> Result<Option<uuid::Uuid>>;
    async fn get_user_vpn_connection(
        &self,
        user_id: &uuid::Uuid,
        limit: i32,
    ) -> Result<Option<Vec<String>>>;
    async fn get_user_traffic_stat(
        &self,
        user_id: &uuid::Uuid,
    ) -> Result<Option<Vec<(uuid::Uuid, ConnStat)>>>;
    async fn get_users(&self) -> Result<Vec<(uuid::Uuid, User)>>;
}

#[async_trait]
impl ApiRequests for BotState {
    async fn register_user(&self, username: &str) -> Result<Option<uuid::Uuid>> {
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
            let resp = res.text().await?;
            let body: ResponseMessage<uuid::Uuid> = serde_json::from_str(&resp)?;
            return Ok(Some(body.message));
        } else if res.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        } else {
            return Err(PonyError::Custom(format!(
                "/user/register req error: {} {:?}",
                res.status(),
                res
            ))
            .into());
        }
    }

    async fn get_user_vpn_connection(
        &self,
        user_id: &uuid::Uuid,
        limit: i32,
    ) -> Result<Option<Vec<String>>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?;

        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Cannot modify API endpoint path".to_string()))?
            .push("user")
            .push("connection");

        let query = UserConnQueryParam {
            user_id: *user_id,
            env: "all".to_string(),
            password: None,
            limit: limit,
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
        user_id: &uuid::Uuid,
    ) -> Result<Option<Vec<(uuid::Uuid, ConnStat)>>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?;

        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Cannot modify API endpoint path".to_string()))?
            .push("user")
            .push("stat");

        let query = UserIdQueryParam { user_id: *user_id };

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

    async fn get_users(&self) -> Result<Vec<(uuid::Uuid, User)>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?;

        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Cannot modify API endpoint path".to_string()))?
            .push("users");

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
            let data: ResponseMessage<Vec<(uuid::Uuid, User)>> = serde_json::from_str(&body)?;

            Ok(data.message)
        } else {
            return Err(
                PonyError::Custom(format!("/users req error: {} {:?}", res.status(), res)).into(),
            );
        }
    }
}
