use async_trait::async_trait;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;

use pony::http::ResponseMessage;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::{PonyError, Result};

use super::Agent;

#[async_trait]
pub trait ApiRequests {
    async fn register_node(&self, _endpoint: String, _token: String) -> Result<()>;
}

#[async_trait]
impl<T, C> ApiRequests for Agent<T, C>
where
    T: NodeStorageOp + Send + Sync + Clone,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    async fn register_node(&self, endpoint: String, token: String) -> Result<()> {
        let node = {
            let state = self.state.lock().await;
            state
                .nodes
                .get_self()
                .expect("No node available to register")
                .clone()
        };

        let mut endpoint_url = Url::parse(&endpoint)?;
        endpoint_url
            .path_segments_mut()
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?
            .push("node");

        let endpoint_str = endpoint_url.to_string();

        match serde_json::to_string_pretty(&node) {
            Ok(json) => log::debug!("Serialized node for environment '{}': {}", node.env, json),
            Err(e) => log::error!("Error serializing node '{}': {}", node.hostname, e),
        }

        let res = HttpClient::new()
            .post(&endpoint_str)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token))
            .json(&node)
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;

        if status.is_success() || status == StatusCode::NOT_MODIFIED {
            log::debug!("Node is already registered: {:?}", status);
            Ok(())
        } else {
            log::error!("Registration failed: {} - {}", status, body);
            Err(PonyError::Custom(
                format!("Registration failed: {} - {}", status, body).into(),
            ))
        }
    }
}
