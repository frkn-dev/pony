use reqwest::Client;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter;

use pony::config::settings::ApiAccessConfig;
use pony::http::filters as pony_filters;
use pony::memory::node::Node;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::BaseConnection as Connection;
use pony::ConnectionBaseOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::Subscription;
use pony::SubscriptionOp;

use crate::core::email::EmailStore;
use crate::core::handlers::auth_handler;
use crate::core::handlers::trial_handler;

pub mod email;
pub mod filters;
pub mod handlers;
pub mod helpers;
pub mod http;
pub mod request;
pub mod response;
pub mod service;
pub mod tasks;

pub type AuthServiceState = MemoryCache<Node, Connection, Subscription>;
pub type HttpClient = Client;

const PROTOS: [&str; 4] = [
    "VlessTcpReality",
    "VlessGrpcReality",
    "VlessXhttpReality",
    "Hysteria2",
];

pub enum Env {
    Dev,
    Ru,
}

impl Display for Env {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Env::Dev => write!(f, "dev"),
            Env::Ru => write!(f, "ru"),
        }
    }
}

const DEFAULT_DAYS: i64 = 1;

pub struct AuthService<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<MemoryCache<N, C, S>>>,
    pub subscriber: ZmqSubscriber,
    pub email_store: EmailStore,
    pub http_client: HttpClient,
    pub ipaddr: Ipv4Addr,
    pub port: u16,
    pub api: ApiAccessConfig,
}

impl<N, C, S> AuthService<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static + Display + PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq + serde::Serialize,
{
    pub fn new(
        memory: Arc<RwLock<MemoryCache<N, C, S>>>,
        subscriber: ZmqSubscriber,
        email_store: EmailStore,
        http_client: HttpClient,
        addr: Ipv4Addr,
        port: u16,
        api: ApiAccessConfig,
    ) -> Self {
        Self {
            memory,
            subscriber,
            email_store,
            http_client,
            ipaddr: addr,
            port,
            api,
        }
    }

    pub async fn start_auth_server(&self) {
        let health_check = warp::path("health-check").map(|| "Server OK");

        let cors = warp::cors()
            .allow_any_origin()
            .allow_methods(vec!["POST"])
            .allow_headers(vec!["Content-Type"]);

        let email_store = self.email_store.clone();
        let memory = self.memory.clone();
        let http_client = self.http_client.clone();
        let api = self.api.clone();

        let trial_route = warp::post()
            .and(warp::path("trial"))
            .and(warp::body::json::<request::Trial>())
            .and(filters::with_store(email_store))
            .and(pony_filters::with_http_client(http_client))
            .and(filters::with_api_settings(api))
            .and_then(trial_handler);

        let auth_route = warp::post()
            .and(warp::path("auth"))
            .and(warp::body::json::<request::Auth>())
            .and(warp::any().map(move || memory.clone()))
            .and_then(auth_handler);

        let routes = health_check.or(auth_route).or(trial_route).with(&cors);

        warp::serve(routes)
            .run(SocketAddr::new(IpAddr::V4(self.ipaddr), self.port))
            .await;
    }
}
