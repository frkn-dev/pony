use async_trait::async_trait;
use std::sync::Arc;
use warp::Filter;

use pony::http::filters::auth;
use pony::http::requests::*;
use pony::state::Conn;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::NodeStorage;
use pony::state::User;
use pony::Result;

use super::super::Api;
use super::filters::*;
use super::handlers::*;
use super::rejection;

#[async_trait]
pub trait Http {
    async fn run(&self) -> Result<()>;
}

#[async_trait]
impl<T, C> Http for Api<T, C>
where
    C: ConnApiOp
        + ConnBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Conn>
        + std::fmt::Debug
        + serde::Serialize,
    T: NodeStorage + Send + Sync + Clone,
{
    async fn run(&self) -> Result<()> {
        let auth = auth(Arc::new(self.settings.api.token.clone()));

        let get_healthcheck_route = warp::get()
            .and(warp::path("healthcheck"))
            .and(with_state(self.state.clone()))
            .and_then(healthcheck_handler);

        let get_user_connections_route = warp::get()
            .and(warp::path("user"))
            .and(warp::path("connections"))
            .and(auth.clone())
            .and(warp::query::<UserIdQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(get_user_connections_handler);

        let get_connection_route = warp::get()
            .and(warp::path("connection"))
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(connections_lines_handler);

        let get_nodes_route = warp::get()
            .and(warp::path("nodes"))
            .and(auth.clone())
            .and(warp::query::<NodesQueryParams>())
            .and(with_state(self.state.clone()))
            .and_then(get_nodes_handler);

        let get_user_conn_stat_route = warp::get()
            .and(warp::path("user"))
            .and(warp::path("stat"))
            .and(auth.clone())
            .and(warp::query::<UserIdQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(user_conn_stat_handler);

        let get_subscription_route = warp::get()
            .and(warp::path("sub"))
            .and(warp::query::<UserSubQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(subscription_link_handler);

        let get_users_route = warp::get()
            .and(warp::path("users"))
            .and(auth.clone())
            .and(with_state(self.state.clone()))
            .and_then(get_users_handler);

        let post_connection_route = warp::post()
            .and(warp::path("connection"))
            .and(auth.clone())
            .and(warp::body::json())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.state.clone()))
            .and_then(create_or_update_connection_handler);

        let post_node_register_route = warp::post()
            .and(warp::path("node"))
            .and(warp::path("register"))
            .and(auth.clone())
            .and(warp::body::json::<NodeRequest>())
            .and(with_state(self.state.clone()))
            .and(publisher(self.publisher.clone()))
            .and_then(node_register_handler);

        let post_user_register_route = warp::post()
            .and(warp::path("user"))
            .and(warp::path("register"))
            .and(auth.clone())
            .and(warp::body::json::<User>())
            .and(with_state(self.state.clone()))
            .and(publisher(self.publisher.clone()))
            .and_then(user_register_handler);

        let post_user_all_connections_route = warp::post()
            .and(warp::path("user"))
            .and(warp::path("connections"))
            .and(auth.clone())
            .and(warp::body::json::<UserIdQueryParam>())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.state.clone()))
            .and_then(create_all_connections_handler);

        let delete_user_route = warp::delete()
            .and(warp::path("user"))
            .and(auth.clone())
            .and(warp::body::json::<UserIdQueryParam>())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.state.clone()))
            .and_then(delete_user_handler);

        let routes = get_healthcheck_route
            .or(get_user_connections_route)
            .or(get_connection_route)
            .or(get_nodes_route)
            .or(get_user_conn_stat_route)
            .or(get_subscription_route)
            .or(get_users_route)
            .or(post_node_register_route)
            .or(post_user_register_route)
            .or(post_user_all_connections_route)
            .or(post_connection_route)
            .or(delete_user_route)
            .recover(rejection);

        if let Some(ipv4) = self.settings.api.address {
            warp::serve(routes)
                .run((ipv4, self.settings.api.port))
                .await;
        }
        Ok(())
    }
}
