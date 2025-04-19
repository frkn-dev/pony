use std::{net::Ipv4Addr, sync::Arc};

use log::debug;
use tokio::sync::Mutex;
use warp::Filter;

use super::handlers::{self, AuthError, NodesQueryParams, UserQueryParams};
use crate::postgres::DbContext;
use crate::state::state::NodeStorage;
use crate::state::{node::NodeRequest, state::State};
use crate::ZmqPublisher;

pub async fn run_api_server<T>(
    state: Arc<Mutex<State<T>>>,
    db: DbContext,
    publisher: ZmqPublisher,
    listen: Ipv4Addr,
    port: u16,
    token: String,
    limit: i64,
) where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    let auth = warp::header::<String>("authorization").and_then(move |auth_header: String| {
        let expected_token = format!("Bearer {}", token.clone());
        debug!("Received Token: {}", auth_header);
        if auth_header == expected_token {
            futures::future::ok(())
        } else {
            futures::future::err(warp::reject::custom(AuthError("Unauthorized".to_string())))
        }
    });

    let user_route = warp::post()
        .and(warp::path("user"))
        .and(auth.clone())
        .and(warp::body::json())
        .and(with_publisher(publisher.clone()))
        .and(with_state(state.clone()))
        .and_then(move |_auth, user_req, publisher, state| {
            handlers::user_request(user_req, publisher, state, limit)
        });

    let nodes_route = warp::get()
        .and(warp::path("nodes"))
        .and(auth.clone())
        .and(warp::query::<NodesQueryParams>())
        .and(with_state(state.clone()))
        .and_then(|_auth, node_req, state| handlers::get_nodes(node_req, state));

    let conn_route = warp::get()
        .and(warp::path("conn"))
        .and(auth.clone())
        .and(warp::query::<UserQueryParams>())
        .and(with_state(state.clone()))
        .and_then(|_auth, user_req, state| handlers::get_conn(user_req, state));

    let nodes_register_route = warp::post()
        .and(warp::path("node"))
        .and(warp::path("register"))
        .and(auth)
        .and(warp::body::json::<NodeRequest>())
        .and(with_state(state))
        .and(with_db(db))
        .and_then(|_auth, node_req, state, db| handlers::node_register(node_req, state, db));

    let routes = user_route
        .or(nodes_route)
        .or(nodes_register_route)
        .or(conn_route)
        .recover(handlers::rejection);

    warp::serve(routes).run((listen, port)).await;
}

fn with_state<T>(
    state: Arc<Mutex<State<T>>>,
) -> impl Filter<Extract = (Arc<Mutex<State<T>>>,), Error = std::convert::Infallible> + Clone
where
    T: NodeStorage + Sync + Send + Clone + 'static,
{
    warp::any().map(move || state.clone())
}

fn with_db(
    db: DbContext,
) -> impl Filter<Extract = (DbContext,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn with_publisher(
    publisher: ZmqPublisher,
) -> impl Filter<Extract = (ZmqPublisher,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || publisher.clone())
}
