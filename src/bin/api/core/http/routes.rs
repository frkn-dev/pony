use async_trait::async_trait;
use std::sync::Arc;
use warp::Filter;

use pony::http::filters::auth;
use pony::http::requests::*;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::Result;

use super::super::Api;
use super::filters::*;
use super::handlers::connection::*;
use super::handlers::node::*;
use super::handlers::sub::*;
use super::rejection;

use crate::core::http::handlers::healthcheck_handler;

#[async_trait]
pub trait Http {
    async fn run(&self, host: String) -> Result<()>;
}

#[async_trait]
impl<N, C> Http for Api<N, C>
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
{
    async fn run(&self, host: String) -> Result<()> {
        let auth = auth(Arc::new(self.settings.api.token.clone()));

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

        let get_node_score_route = warp::get()
            .and(warp::path("node"))
            .and(warp::path("score"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<NodeIdParam>())
            .and(with_state(self.sync.clone()))
            .and(with_ch(self.ch.clone()))
            .and_then(get_node_score_handler);

        let post_node_register_route = warp::post()
            .and(warp::path("node"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::body::json::<NodeRequest>())
            .and(warp::query::<NodeTypeParam>())
            .and(with_state(self.sync.clone()))
            .and(publisher(self.publisher.clone()))
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
            .and(with_param_string(host))
            .and_then(subscription_info_handler::<C>);

        // Connections Routes

        let get_connection_route = warp::get()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(with_state(self.sync.clone()))
            .and_then(get_connection_handler);

        let post_connection_route = warp::post()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::body::json())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.sync.clone()))
            .and_then(create_connection_handler);

        let delete_connection_route = warp::delete()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.sync.clone()))
            .and_then(delete_connection_handler);

        let put_connection_route = warp::put()
            .and(warp::path("connection"))
            .and(warp::path::end())
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(warp::body::json())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.sync.clone()))
            .and_then(put_connection_handler);

        let routes = get_healthcheck_route
            // Subscription
            .or(get_subscription_connections_route)
            .or(get_subscription_stat_route)
            .or(get_subscription_route)
            .or(get_subscription_info_route)
            // Node
            .or(get_nodes_route)
            .or(get_node_route)
            .or(get_node_score_route)
            .or(post_node_register_route)
            // Connection
            .or(get_connection_route)
            .or(post_connection_route)
            .or(delete_connection_route)
            .or(put_connection_route)
            .recover(rejection);

        if let Some(ipv4) = self.settings.api.address {
            warp::serve(routes)
                .run((ipv4, self.settings.api.port))
                .await;
        }
        Ok(())
    }
}
