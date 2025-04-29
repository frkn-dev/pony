use async_trait::async_trait;
use std::sync::Arc;
use warp::Filter;

use pony::http::requests::NodeRequest;
use pony::http::requests::NodesQueryParams;
use pony::http::requests::UserQueryParam;
use pony::state::connection::Conn;
use pony::state::connection::ConnApiOp;
use pony::state::connection::ConnBaseOp;
use pony::state::state::NodeStorage;
use pony::Result;

use super::super::Api;
use super::filters::*;
use super::handlers::*;

#[async_trait]
pub trait Http {
    async fn run(&self) -> Result<()>;
}

#[async_trait]
impl<T, C> Http for Api<T, C>
where
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
    T: NodeStorage + Send + Sync + Clone,
{
    async fn run(&self) -> Result<()> {
        let auth = auth(Arc::new(self.settings.api.token.clone()));
        let limit = self.settings.api.conn_limit_mb;

        // let connection_get_route = warp::get()
        //     .and(warp::path("connection"))
        //     .and(auth.clone())
        //     .and(warp::query::<UserQueryParam>())
        //     .and(with_state(self.state.clone()))
        //     .and(publisher(self.publisher.clone()))
        //     .and(db(self.db.clone()))
        //     .and_then(|conn_req, state, publisher, db| {
        //         connections_lines_handler(conn_req, state, publisher, db)
        //     });
        //
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
            .and(db(self.db.clone()))
            .and(publisher(self.publisher.clone()))
            .and_then(|node_req, state, db, publisher| {
                node_register(node_req, state, db, publisher)
            });

        let user_register_route = warp::post()
            .and(warp::path("user"))
            .and(warp::path("register"))
            .and(auth)
            .and(warp::body::json::<UserQueryParam>())
            .and(with_state(self.state.clone()))
            .and(db(self.db.clone()))
            .and_then(|user_req, state, db| user_register(user_req, state, db));

        let routes = connection_post_route
            .or(nodes_get_route)
            .or(node_register_route)
            .or(user_register_route)
            .recover(rejection);

        if let Some(ipv4) = self.settings.api.address {
            warp::serve(routes)
                .run((ipv4, self.settings.api.port))
                .await;
        }
        Ok(())
    }
}
