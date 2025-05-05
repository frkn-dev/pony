use async_trait::async_trait;
use std::sync::Arc;
use warp::Filter;

use pony::http::requests::*;
use pony::state::Conn;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::NodeStorage;
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
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn> + std::fmt::Debug,
    T: NodeStorage + Send + Sync + Clone,
{
    async fn run(&self) -> Result<()> {
        let auth = auth(Arc::new(self.settings.api.token.clone()));
        let limit = self.settings.api.conn_limit_mb;

        let user_connection_get_route = warp::get()
            .and(warp::path("user"))
            .and(warp::path("connection"))
            .and(auth.clone())
            .and(warp::query::<UserConnQueryParam>())
            .and(with_state(self.state.clone()))
            .and(publisher(self.publisher.clone()))
            .and_then(|conn_req, sync_state, publisher| {
                user_connections_lines_handler(conn_req, sync_state, publisher)
            });

        let connection_get_route = warp::get()
            .and(warp::path("connection"))
            .and(auth.clone())
            .and(warp::query::<ConnQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(|conn_req, state| connections_lines_handler(conn_req, state));

        let connection_post_route = warp::post()
            .and(warp::path("connection"))
            .and(auth.clone())
            .and(warp::body::json())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.state.clone()))
            .and_then(move |conn_req, publisher, state| {
                create_connection_handler(conn_req, publisher, state, limit)
            });

        let nodes_get_route = warp::get()
            .and(warp::path("nodes"))
            .and(auth.clone())
            .and(warp::query::<NodesQueryParams>())
            .and(with_state(self.state.clone()))
            .and_then(|node_req, state| get_nodes_handler(node_req, state));

        let node_register_route = warp::post()
            .and(warp::path("node"))
            .and(warp::path("register"))
            .and(auth.clone())
            .and(warp::body::json::<NodeRequest>())
            .and(with_state(self.state.clone()))
            .and(publisher(self.publisher.clone()))
            .and_then(|node_req, state, publisher| node_register(node_req, state, publisher));

        let user_conn_stat_route = warp::get()
            .and(warp::path("user"))
            .and(warp::path("stat"))
            .and(auth.clone())
            .and(warp::query::<UserIdQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(|user_req, sync_state| user_conn_stat_handler(user_req, sync_state));

        let user_register_route = warp::post()
            .and(warp::path("user"))
            .and(warp::path("register"))
            .and(auth.clone())
            .and(warp::body::json::<UserRegQueryParam>())
            .and(with_state(self.state.clone()))
            .and_then(|user_req, sync_state| user_register_handler(user_req, sync_state));

        let users_route = warp::get()
            .and(warp::path("users"))
            .and(auth)
            .and(with_state(self.state.clone()))
            .and_then(|sync_state| get_users_handler(sync_state));

        let routes = connection_post_route
            .or(user_connection_get_route)
            .or(connection_get_route)
            .or(nodes_get_route)
            .or(node_register_route)
            .or(user_register_route)
            .or(user_conn_stat_route)
            .or(users_route)
            .recover(rejection);

        if let Some(ipv4) = self.settings.api.address {
            warp::serve(routes)
                .run((ipv4, self.settings.api.port))
                .await;
        }
        Ok(())
    }
}
