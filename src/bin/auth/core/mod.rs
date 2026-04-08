use reqwest::Client;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter;

use pony::config::settings::ApiAccessConfig;
use pony::http::filters as pony_filters;
use pony::memory::connection::Connections;
use pony::memory::node::Node;
use pony::metrics::storage::MetricBuffer;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::ConnectionBaseOp;

use crate::core::email::EmailStore;
use crate::core::handlers::trial_handler;
use crate::core::handlers::{activate_key_handler, auth_handler};

pub mod email;
pub mod filters;
pub mod handlers;
pub mod helpers;
pub mod http;
pub mod metrics;
pub mod request;
pub mod response;
pub mod service;
pub mod tasks;

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
    Wl,
}

impl Display for Env {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Env::Dev => write!(f, "dev"),
            Env::Ru => write!(f, "ru"),
            Env::Wl => write!(f, "wl"),
        }
    }
}

const DEFAULT_DAYS: i64 = 1;

pub struct AuthService<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<Connections<C>>>,
    pub metrics: Arc<MetricBuffer>,
    pub node: Node,
    pub subscriber: ZmqSubscriber,
    pub email_store: EmailStore,
    pub http_client: HttpClient,
    pub api: ApiAccessConfig,
    pub listen: Ipv4Addr,
    pub port: u16,
}

impl<C> AuthService<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static + std::fmt::Display,
{
    pub fn new(
        metrics: Arc<MetricBuffer>,
        node: Node,
        subscriber: ZmqSubscriber,
        email_store: EmailStore,
        http_client: HttpClient,
        api: ApiAccessConfig,
        listen: (Ipv4Addr, u16),
    ) -> Self {
        let memory = Arc::new(RwLock::new(Connections::default()));
        Self {
            memory,
            metrics,
            node,
            subscriber,
            email_store,
            http_client,
            api,
            listen: listen.0,
            port: listen.1,
        }
    }

    pub async fn start_auth_server(&self) {
        let health_check = warp::path("health-check").map(|| "Server OK");

        let cors = warp::cors()
            .allow_origin("http://localhost:8000")
            .allow_origin("https://frkn.org")
            .allow_credentials(true)
            .allow_methods(vec!["GET", "POST", "OPTIONS"])
            .allow_headers(vec!["Content-Type"])
            .max_age(86400)
            .build();

        let email_store = self.email_store.clone();
        let memory = self.memory.clone();
        let http_client = self.http_client.clone();
        let api = self.api.clone();

        let trial_route = warp::post()
            .and(warp::path("trial"))
            .and(warp::body::json::<request::Trial>())
            .and(filters::with_store(email_store.clone()))
            .and(pony_filters::with_http_client(http_client.clone()))
            .and(filters::with_api_settings(api.clone()))
            .and_then(trial_handler)
            .with(&cors);

        let auth_route = warp::post()
            .and(warp::path("auth"))
            .and(warp::body::json::<request::Auth>())
            .and(warp::any().map(move || memory.clone()))
            .and_then(auth_handler);

        let activate_route = warp::post()
            .and(warp::path("activate"))
            .and(warp::body::json::<request::Key>())
            .and(filters::with_store(email_store))
            .and(pony_filters::with_http_client(http_client))
            .and(filters::with_api_settings(api))
            .and_then(activate_key_handler)
            .with(&cors);

        let routes = health_check
            .or(auth_route)
            .or(trial_route)
            .or(activate_route);

        warp::serve(routes)
            .run(SocketAddr::new(IpAddr::V4(self.listen), self.port))
            .await;
    }
}
