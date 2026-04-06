use async_trait::async_trait;
use std::sync::Arc;
use warp::Filter;

use pony::config::settings::ApiServiceConfig;
use pony::http::filters::{auth, with_i64};
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::Result;
use pony::SubscriptionOp;

use super::super::Api;
use super::filters::*;
use super::handlers::connection::*;
use super::handlers::key::*;
use super::handlers::metrics::*;
use super::handlers::node::*;
use super::handlers::sub::*;
use super::param::*;
use super::rejection;
use super::request::*;

use crate::core::http::handlers::healthcheck_handler;

#[async_trait]
pub trait Http {
    async fn run(&self, params: ApiServiceConfig) -> Result<()>;
}

#[async_trait]
impl<N, C, S> Http for Api<N, C, S>
where
    C: ConnectionBaseOp
        + ConnectionApiOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + std::fmt::Debug
        + serde::Serialize
        + PartialEq,
    N: NodeStorageOp + Send + Sync + Clone,
    Connection: From<C>,

    S: SubscriptionOp
        + Send
        + Sync
        + Clone
        + 'static
        + std::cmp::PartialEq
        + std::convert::From<pony::Subscription>,
{
    async fn run(&self, params: ApiServiceConfig) -> Result<()> {
        let auth = auth(Arc::new(self.settings.api.token.clone()));

        let cors = warp::cors()
            .allow_any_origin()
            .allow_methods(vec!["GET", "POST", "DELETE"]);

        let get_healthcheck_route = warp::get()
            .and(warp::path("healthcheck"))
            .and(warp::path::end())
            .and(with_state(self.sync.clone()))
            .and_then(healthcheck_handler);

        // Node routes
        let get_nodes_route = warp::get()
            .and(warp::path("nodes"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<NodesQueryParams>())
            .and(with_state(self.sync.clone()))
            .and_then(get_nodes_handler);

        let post_node_register_route = warp::post()
            .and(warp::path("node"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::body::json::<NodeRequest>())
            .and(with_state(self.sync.clone()))
            .and_then(post_node_handler);

        let get_node_route = warp::get()
            .and(warp::path("node"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<NodeIdParam>())
            .and(with_state(self.sync.clone()))
            .and_then(get_node_handler);

        // Subscription Routes

        let get_subscription_stat_route = warp::get()
            .and(warp::path("sub"))
            .and(warp::path("stat"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<SubIdQueryParam>())
            .and(with_state(self.sync.clone()))
            .and_then(subscription_conn_stat_handler);

        let get_subscription_connections_route = warp::get()
            .and(warp::path("sub"))
            .and(warp::path("connections"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<SubIdQueryParam>())
            .and(with_state(self.sync.clone()))
            .and_then(get_subscription_connections_handler);

        let get_subscription_route = warp::get()
            .and(warp::path("sub"))
            .and(warp::path::end())
            .and(warp::query::<SubQueryParam>())
            .and(with_state(self.sync.clone()))
            .and_then(subscription_link_handler);

        let get_subscription_info_route = warp::get()
            .and(warp::path("sub"))
            .and(warp::path("info"))
            .and(warp::path::end())
            .and(warp::query::<SubQueryParam>())
            .and(with_state(self.sync.clone()))
            .and(with_param_string(params.web_host))
            .and(with_param_string(params.api_web_host))
            .and_then(subscription_info_handler);

        let post_subscription_route = warp::post()
            .and(warp::path("subscription"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::body::json())
            .and(with_state(self.sync.clone()))
            .and(with_i64(params.bonus_days))
            .and(with_param_vec_string(params.promo_codes))
            .and_then(post_subscription_handler);

        let put_subscription_route = warp::put()
            .and(warp::path("subscription"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<SubIdQueryParam>())
            .and(warp::body::json())
            .and(with_state(self.sync.clone()))
            .and_then(put_subscription_handler);

        // Connections Routes

        let get_connection_route = warp::get()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(with_state(self.sync.clone()))
            .and_then(get_connection_handler);

        let get_connections_route = warp::get()
            .and(warp::path("connections"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<ConnTypeParam>())
            .and(with_state(self.sync.clone()))
            .and_then(get_connections_handler);

        let post_connection_route = warp::post()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::body::json())
            .and(with_state(self.sync.clone()))
            .and_then(create_connection_handler);

        let delete_connection_route = warp::delete()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(with_state(self.sync.clone()))
            .and_then(delete_connection_handler);

        // Keys Routes
        let get_key_validation_route = warp::get()
            .and(warp::path("key"))
            .and(warp::path("validate"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<KeyQueryParams>())
            .and(with_state(self.sync.clone()))
            .and(with_param_vec(params.key_sign_token.clone()))
            .and_then(get_key_validate_handler);

        let post_key_route = warp::post()
            .and(warp::path("key"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::body::json())
            .and(with_state(self.sync.clone()))
            .and(with_param_vec(params.key_sign_token))
            .and_then(post_key_handler);

        let post_activate_key_route = warp::post()
            .and(warp::path("key"))
            .and(warp::path("activate"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_state(self.sync.clone()))
            .and_then(post_activate_key_handler);

        let get_metrics_route = warp::path!("api" / "metrics" / String / "heartbeat")
            .and(warp::get())
            .and(with_metrics(self.metrics.clone())) // прокидываем сторадж
            .and_then(get_heartbeat_handler);

        let routes = get_healthcheck_route
            // Subscription
            .or(get_subscription_connections_route)
            .or(get_subscription_stat_route)
            .or(get_subscription_route)
            .or(get_subscription_info_route)
            .or(post_subscription_route)
            .or(put_subscription_route)
            // Node
            .or(get_nodes_route)
            .or(get_node_route)
            .or(post_node_register_route)
            // Connection
            .or(get_connection_route)
            .or(get_connections_route)
            .or(post_connection_route)
            .or(delete_connection_route)
            // Key
            .or(get_key_validation_route)
            .or(post_key_route)
            .or(post_activate_key_route)
            // Metrics
            .or(get_metrics_route)
            .recover(rejection)
            .with(cors);

        if let Some(ipv4) = self.settings.api.address {
            warp::serve(routes)
                .run((ipv4, self.settings.api.port))
                .await;
        }
        Ok(())
    }
}
