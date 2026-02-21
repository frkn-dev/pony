use pony::http::requests::ConnTypeParam;
use pony::memory::cache::Cache;
use pony::ConnectionBaseOp;
use pony::ConnectionStorageBaseOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;
use pony::Tag;
use serde::Deserialize;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter;

#[derive(Deserialize)]
pub struct AuthRequest {
    addr: String,
    pub auth: uuid::Uuid,
    tx: u64,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

pub async fn start_auth_server<N, C, S>(
    memory: Arc<RwLock<Cache<N, C, S>>>,
    ipaddr: Ipv4Addr,
    port: u16,
) where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    S: SubscriptionOp + Sync + Send + Clone + 'static + std::cmp::PartialEq + serde::Serialize,
    C: ConnectionBaseOp + Sync + Send + Clone + 'static + std::fmt::Display,
{
    let health_check = warp::path("health-check").map(|| "Server OK");

    let auth_route = warp::post()
        .and(warp::path("auth"))
        .and(warp::body::json())
        .and(warp::any().map(move || memory.clone()))
        .and_then(auth_handler);

    let routes = health_check
        .or(auth_route)
        .with(warp::cors().allow_any_origin());

    warp::serve(routes)
        .run(SocketAddr::new(IpAddr::V4(ipaddr), port))
        .await;
}

pub async fn auth_handler<N, S, C>(
    req: AuthRequest,
    memory: Arc<RwLock<Cache<N, C, S>>>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    S: SubscriptionOp + Sync + Send + Clone + 'static + std::cmp::PartialEq + serde::Serialize,
    C: ConnectionBaseOp + Sync + Send + Clone + 'static + std::fmt::Display,
{
    log::debug!("Auth req {} {} {}", req.auth, req.addr, req.tx);
    let mem = memory.read().await;
    if let Some(id) = mem.connections.validate_token(&req.auth) {
        return Ok(warp::reply::json(&AuthResponse {
            ok: true,
            id: Some(id.to_string()),
        }));
    } else {
        return Ok(warp::reply::json(&AuthResponse {
            ok: false,
            id: None,
        }));
    }
}

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use reqwest::Url;

use pony::{PonyError, Result as PonyResult};

use super::AuthService;

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
impl<T, C, S> ApiRequests for AuthService<T, C, S>
where
    T: NodeStorageOp + Send + Sync + Clone,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    async fn get_connections(
        &self,
        endpoint: String,
        token: String,
        proto: Tag,
        last_update: Option<u64>,
    ) -> PonyResult<()> {
        let node = {
            let mem = self.memory.read().await;
            mem.nodes
                .get_self()
                .expect("No node available to register")
                .clone()
        };

        let env = node.env;

        let conn_type_param = ConnTypeParam {
            proto: proto,
            last_update: last_update,
            env: env,
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
            log::debug!("Connections Request Accepted: {:?}", status);
            Ok(())
        } else {
            log::error!("Connections Request failed: {} - {}", status, body);
            Err(PonyError::Custom(
                format!("Connections Request failed: {} - {}", status, body).into(),
            ))
        }
    }
}
