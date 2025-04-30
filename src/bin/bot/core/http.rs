use async_trait::async_trait;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;

use pony::http::requests::UserQueryParam;
use pony::postgres::connection::ConnRow;
use pony::{PonyError, Result};

use super::BotState;

pub enum RegisterStatus {
    Ok,
    AlreadyExist,
}

#[async_trait]
pub trait ApiRequests {
    async fn register_user(&self, _username: &str) -> Result<RegisterStatus>;

    async fn create_vpn_connection(&self, _conn_id: &uuid::Uuid) -> Result<()>;

    async fn get_user_vpn_connection(&self, username: &str) -> Result<Option<Vec<String>>>;
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

        let user = UserQueryParam {
            username: username.to_string(),
            env: "all".to_string(),
            password: None,
            limit: 1024,
            trial: true,
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

    async fn create_vpn_connection(&self, conn_id: &uuid::Uuid) -> Result<()> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)?;
        endpoint
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?
            .push("connection");
        let endpoint = endpoint.to_string();

        let conn = ConnRow::new(*conn_id);
        let conn_req = conn.as_create_conn_request();
        let json = serde_json::to_string_pretty(&conn_req).unwrap();

        log::debug!("create_vpn_connection JSON {}", json);

        let res = HttpClient::new()
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", &self.settings.api.token),
            )
            .json(&conn_req)
            .send()
            .await?;

        if res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED {
            return Ok(());
        } else {
            return Err(PonyError::Custom(format!(
                "/connection req error: {} {:?}",
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

        let query = UserQueryParam {
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
                "get_vpn_connection request error: {}",
                res.status()
            ))
            .into()),
        }
    }
}
