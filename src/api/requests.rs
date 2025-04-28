use async_trait::async_trait;
use log::debug;
use log::error;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;
use std::error::Error;
use uuid::Uuid;

use crate::bot::BotState;
use crate::http::handlers::ResponseMessage;
use crate::Agent;
use crate::ConnRow;
use crate::NodeStorage;

#[async_trait]
pub trait ApiRequests {
    async fn register_node(
        &self,
        _endpoint: String,
        _token: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Err("register_node not implemented".into())
    }

    async fn create_vpn_connection(
        &self,
        _conn_id: Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Err("create_vpn_connection not implemented".into())
    }

    async fn get_vpn_connection(
        &self,
        _conn_id: Uuid,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        Err("get_vpn_connection not implemented".into())
    }
}

#[async_trait]
impl<T: NodeStorage + Send + Sync + Clone> ApiRequests for Agent<T> {
    async fn register_node(
        &self,
        endpoint: String,
        token: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let node = {
            let state = self.state.lock().await;
            state
                .nodes
                .get()
                .expect("No node available to register")
                .clone()
        };

        let mut endpoint_url = Url::parse(&endpoint)?;
        endpoint_url
            .path_segments_mut()
            .map_err(|_| "Invalid API endpoint")?
            .push("node")
            .push("register");

        let endpoint_str = endpoint_url.to_string();
        debug!("ENDPOINT: {}", endpoint_str);

        match serde_json::to_string_pretty(&node) {
            Ok(json) => debug!("Serialized node for environment '{}': {}", node.env, json),
            Err(e) => error!("Error serializing node '{}': {}", node.hostname, e),
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
            if body.trim().is_empty() {
                debug!("Node is already registered");
                Ok(())
            } else {
                let parsed: ResponseMessage<String> = serde_json::from_str(&body)?;
                debug!("Node is already registered: {:?}", parsed);
                Ok(())
            }
        } else {
            error!("Registration failed: {} - {}", status, body);
            Err(format!("Registration failed: {} - {}", status, body).into())
        }
    }
}

#[async_trait]
impl ApiRequests for BotState {
    async fn create_vpn_connection(
        &self,
        conn_id: uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)?;
        endpoint
            .path_segments_mut()
            .map_err(|_| "Invalid API endpoint")?
            .push("user");
        let endpoint = endpoint.to_string();

        debug!("ENDPOINT: {}", endpoint);

        let conn = ConnRow::new(conn_id);
        let conn_req = conn.as_create_conn_request();
        let json = serde_json::to_string_pretty(&conn_req).unwrap();

        println!("create_vpn_connection JSON {}", json);

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
            return Err(format!(
                "create_vpn_connection Req error: {} {:?}",
                res.status(),
                res
            )
            .into());
        }
    }

    async fn get_vpn_connection(
        &self,
        conn_id: uuid::Uuid,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let mut endpoint = Url::parse(&self.settings.api.endpoint)?;
        endpoint
            .path_segments_mut()
            .map_err(|_| "Invalid API endpoint")?
            .push("conn");

        endpoint
            .query_pairs_mut()
            .append_pair("id", &conn_id.to_string());

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

        if res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED {
            let body = res.text().await?;
            let data: Vec<String> = serde_json::from_str(&body)?;
            return Ok(data);
        } else {
            return Err(format!("get_vpn_connection Req error: {} {:?}", res.status(), res).into());
        }
    }
}
