use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};

use fcore::{
    http::{
        request::ConnType,
        response::{Instance, InstanceWithId, ResponseMessage},
    },
    ConnectionBaseOperations, Error, Result, Tag, Topic,
};

use super::service::Service;
pub type HttpClient = Client;

#[async_trait]
pub trait ApiRequests {
    async fn get_connections(
        &self,
        endpoint: String,
        token: String,
        proto: Tag,
        last_update: Option<u64>,
    ) -> Result<()>;
}

#[async_trait]
impl<C> ApiRequests for Service<C>
where
    C: ConnectionBaseOperations + Send + Sync + Clone + 'static,
{
    async fn get_connections(
        &self,
        endpoint: String,
        token: String,
        proto: Tag,
        last_update: Option<u64>,
    ) -> Result<()> {
        let topic = Topic::Init(self.node.uuid);
        let env = self.node.env.clone();

        let conn_type_param = ConnType {
            proto,
            last_update,
            env: env.clone(),
            topic: topic.clone(),
        };

        let mut endpoint_url = Url::parse(&endpoint).map_err(|e| {
            tracing::error!("Failed to parse endpoint URL '{}': {}", endpoint, e);
            Error::Custom("Invalid API endpoint".to_string())
        })?;

        endpoint_url
            .path_segments_mut()
            .map_err(|_| Error::Custom("Invalid API endpoint".to_string()))?
            .push("connections")
            .push("sync");

        let endpoint_str = endpoint_url.to_string();

        tracing::debug!("POST /connections/sync Body: {:?}", conn_type_param);

        let res = HttpClient::new()
            .post(&endpoint_str)
            .header("Authorization", format!("Bearer {}", token.trim()))
            .json(&conn_type_param)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("CRITICAL: reqwest send error: {:?}", e);
                Error::Custom(format!("HTTP Send Error: {}", e))
            })?;

        let status = res.status();
        let body = res.text().await?;

        if status.is_success() {
            let result: ResponseMessage<InstanceWithId<Instance>> = serde_json::from_str(&body)?;
            let count = match result.response.instance {
                Instance::Count(count) => count,
                _ => return Err(Error::Custom("Unexpected instance type".into())),
            };
            tracing::debug!(
                "Success: {} connections synced for {} {} {}",
                count,
                proto,
                env,
                topic
            );
            Ok(())
        } else if status == StatusCode::NOT_MODIFIED {
            tracing::debug!("No updates (304) for {} {} {}", proto, env, topic);
            Ok(())
        } else {
            tracing::error!("Request failed: {} - {}", status, body);
            Err(Error::Custom(format!("Status {}: {}", status, body)))
        }
    }
}
