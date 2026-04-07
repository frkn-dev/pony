use async_trait::async_trait;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use pony::config::xray::Inbound;
use pony::ConnectionBaseOp;
use pony::Tag;
use pony::{PonyError, Result};
use std::net::Ipv4Addr;

use super::Agent;

use pony::http::response::{Instance, InstanceWithId, ResponseMessage};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnTypeParam {
    pub proto: Tag,
    pub last_update: Option<u64>,
    pub env: Option<String>,
    pub topic: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeRequest {
    pub env: String,
    pub hostname: String,
    pub address: Ipv4Addr,
    pub inbounds: HashMap<Tag, Inbound>,
    pub uuid: uuid::Uuid,
    pub label: String,
    pub interface: String,
    pub cores: usize,
    pub max_bandwidth_bps: i64,
    pub country: String,
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
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
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
                "Message: {}. Connections Request Accepted for {}: {} Count: {} ",
                result.message,
                proto,
                result.status,
                count,
            );
            Ok(())
        } else if status == StatusCode::NOT_MODIFIED {
            log::debug!("Connections Request Accepted for {}: {} ", proto, status,);
            Ok(())
        } else {
            log::error!(
                "Connections Request failed for {}: {} - {}",
                proto,
                status,
                body
            );
            Err(PonyError::Custom(format!(
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
            .map_err(|_| PonyError::Custom("Invalid API endpoint".to_string()))?
            .push("node");
        let endpoint_str = endpoint_url.to_string();

        match serde_json::to_string_pretty(&node) {
            Ok(json) => log::debug!("Serialized node for environment '{}': {}", node.env, json),
            Err(e) => log::error!("Error serializing node '{}': {}", node.hostname, e),
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
            log::debug!("Node is already registered: {:?}", status);
            Ok(())
        } else {
            log::error!("Registration failed: {} - {}", status, body);
            Err(PonyError::Custom(format!(
                "Registration failed: {} - {}",
                status, body
            )))
        }
    }
}
