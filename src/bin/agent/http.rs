use async_trait::async_trait;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use super::agent::Agent;

use pony::http::response::{Instance, InstanceWithId, ResponseMessage};
use pony::{ConnectionBaseOperations, Env, Error, Inbound, NodeType, Result, Tag};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnTypeParam {
    pub proto: Tag,
    pub last_update: Option<u64>,
    pub env: Option<Env>,
    pub topic: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub env: Env,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: uuid::Uuid,
    pub label: String,
    pub interface: String,
    pub cores: usize,
    pub max_bandwidth_bps: i64,
    pub country: String,
    pub r#type: NodeType,
}

#[async_trait]
pub trait ApiRequests {
    async fn register_node(&self, _endpoint: String, _token: String) -> Result<()>;
    async fn get_connections(
        &self,
        endpoint: String,
        token: String,
        proto: Tag,
        last_update: Option<u64>,
    ) -> Result<()>;
}

#[async_trait]
impl<C> ApiRequests for Agent<C>
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
        let node = self.node.clone();

        let id = node.uuid;
        let env = node.env;

        let conn_type_param = ConnTypeParam {
            proto,
            last_update,
            env: Some(env),
            topic: id,
        };

        let mut endpoint_url = Url::parse(&endpoint)?;
        endpoint_url
            .path_segments_mut()
            .map_err(|_| Error::Custom("Invalid API endpoint".to_string()))?
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
                    return Err(Error::Custom("Unexpected instance type".into()));
                }
            };
            tracing::debug!(
                "Message: {}. Connections Request Accepted for {}: {} Count: {} ",
                result.message,
                proto,
                result.status,
                count,
            );
            Ok(())
        } else if status == StatusCode::NOT_MODIFIED {
            tracing::debug!("Connections Request Accepted for {}: {} ", proto, status,);
            Ok(())
        } else {
            tracing::error!(
                "Connections Request failed for {}: {} - {}",
                proto,
                status,
                body
            );
            Err(Error::Custom(format!(
                "Connections Request failed for {}: {} - {}",
                proto, status, body
            )))
        }
    }

    async fn register_node(&self, endpoint: String, token: String) -> Result<()> {
        let node = self.node.clone();

        let mut endpoint_url = Url::parse(&endpoint)?;
        endpoint_url
            .path_segments_mut()
            .map_err(|_| Error::Custom("Invalid API endpoint".to_string()))?
            .push("node");
        let endpoint_str = endpoint_url.to_string();

        match serde_json::to_string_pretty(&node) {
            Ok(json) => tracing::debug!("Serialized node for environment '{}': {}", node.env, json),
            Err(e) => tracing::error!("Error serializing node '{}': {}", node.hostname, e),
        }

        let node_request = NodeRequest {
            env: node.env.clone(),
            hostname: node.hostname.clone(),
            address: node.address,
            inbounds: node.inbounds.clone(),
            uuid: node.uuid,
            label: node.label.clone(),
            interface: node.interface.clone(),
            cores: node.cores,
            max_bandwidth_bps: node.max_bandwidth_bps,
            country: node.country,
            r#type: node.r#type,
        };

        let res = HttpClient::new()
            .post(&endpoint_str)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token))
            .json(&node_request)
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;
        if status.is_success() || status == StatusCode::NOT_MODIFIED {
            tracing::debug!("Node is already registered: {:?}", status);
            Ok(())
        } else {
            tracing::error!("Registration failed: {} - {}", status, body);
            Err(Error::Custom(format!(
                "Registration failed: {} - {}",
                status, body
            )))
        }
    }
}
