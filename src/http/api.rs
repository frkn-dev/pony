use async_trait::async_trait;
use std::sync::Arc;
use warp::Filter;

use super::filters::*;
use super::handlers::*;
use crate::state::node::NodeRequest;
use crate::state::state::NodeStorage;
use crate::Api;

#[async_trait]
pub trait Http {
    async fn run(&self);
}

#[async_trait]
impl<T: NodeStorage + Send + Sync + Clone + 'static> Http for Api<T> {
    async fn run(&self) {
        let auth = auth(Arc::new(self.settings.api.token.clone()));
        let limit = self.settings.api.user_limit_mb;

        let user_route = warp::post()
            .and(warp::path("user"))
            .and(auth.clone())
            .and(warp::body::json())
            .and(publisher(self.publisher.clone()))
            .and(with_state(self.state.clone()))
            .and_then(move |user_req, publisher, state| {
                user_request(user_req, publisher, state, limit)
            });

        let nodes_route = warp::get()
            .and(warp::path("nodes"))
            .and(auth.clone())
            .and(warp::query::<NodesQueryParams>())
            .and(with_state(self.state.clone()))
            .and_then(|node_req, state| get_nodes(node_req, state));

        let conn_route = warp::get()
            .and(warp::path("conn"))
            .and(auth.clone())
            .and(warp::query::<UserQueryParams>())
            .and(with_state(self.state.clone()))
            .and_then(|user_req, state| get_conn(user_req, state));

        let nodes_register_route = warp::post()
            .and(warp::path("node"))
            .and(warp::path("register"))
            .and(auth)
            .and(warp::body::json::<NodeRequest>())
            .and(with_state(self.state.clone()))
            .and(db(self.db.clone()))
            .and(publisher(self.publisher.clone()))
            .and_then(|node_req, state, db, publisher| {
                node_register(node_req, state, db, publisher)
            });

        let routes = user_route
            .or(nodes_route)
            .or(nodes_register_route)
            .or(conn_route)
            .recover(rejection);

        if let Some(ipv4) = self.settings.api.address {
            warp::serve(routes)
                .run((ipv4, self.settings.api.port))
                .await;
        }
    }
}
