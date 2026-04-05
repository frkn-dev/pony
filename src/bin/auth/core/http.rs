use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use pony::ConnectionBaseOp;
use pony::Tag;
use pony::{PonyError, Result as PonyResult};

use super::AuthService;
use super::HttpClient;

use reqwest::Url;

use pony::http::response::{Instance, InstanceWithId, ResponseMessage};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnTypeParam {
    pub proto: Tag,
    pub last_update: Option<u64>,
    pub env: Option<String>,
    pub topic: uuid::Uuid,
}

#[async_trait]
pub trait ApiRequests {
    async fn get_connections(
        &self,
        endpoint: String,
        token: String,
        proto: Tag,
        last_update: Option<u64>,
    ) -> PonyResult<()>;
}

#[async_trait]
impl<C> ApiRequests for AuthService<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    async fn get_connections(
        &self,
        endpoint: String,
        token: String,
        proto: Tag,
        last_update: Option<u64>,
    ) -> PonyResult<()> {
        let id = self.node.uuid;

        let conn_type_param = ConnTypeParam {
            proto,
            last_update,
            env: None,
            topic: id,
        };

        let mut endpoint_url = Url::parse(&endpoint)?;
        endpoint_url
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?
            .push("connections");
        let endpoint_str = endpoint_url.to_string();

        let res = HttpClient::new()
            .get(&endpoint_str)
            .query(&conn_type_param)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;
        if status.is_success() {
            let result: ResponseMessage<InstanceWithId<Instance>> = serde_json::from_str(&body)?;
            let count = match result.response.instance {
                Instance::Count(count) => count,
                _ => {
                    return Err(PonyError::Custom("Unexpected instance type".into()));
                }
            };
            log::debug!(
                "Connections Request Accepted for {}: {} Count: {} ",
                proto,
                status,
                count,
            );
            Ok(())
        } else if status == StatusCode::NOT_MODIFIED {
            log::debug!("Connections Request Accepted for {}: {} ", proto, status,);
            Ok(())
        } else {
            log::error!("Connections Request failed: {} - {}", status, body);
            Err(PonyError::Custom(format!(
                "Connections Request failed: {} - {}",
                status, body
            )))
        }
    }
}
