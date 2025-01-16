use log::debug;
use std::{net::Ipv4Addr, sync::Arc};
use tokio::sync::Mutex;
use tokio_postgres::Client;
use warp::Filter;
use zmq::Socket;

use super::handlers::AuthError;
use super::handlers::{self, NodesQueryParams, UserQueryParams};
use crate::state::{node::NodeRequest, state::State};

pub async fn run_api_server(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<Client>>,
    publisher: Arc<Mutex<Socket>>,
    listen: Ipv4Addr,
    port: u16,
    token: String,
) {
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
        .and_then(|_auth, user_req, publisher, state| {
            handlers::user_request(user_req, publisher, state)
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
        .and(with_pg_client(client))
        .and(with_publisher(publisher))
        .and_then(|_auth, node_req, state, client, publisher| {
            handlers::node_register(node_req, state, client, publisher)
        });

    let routes = user_route
        .or(nodes_route)
        .or(nodes_register_route)
        .or(conn_route)
        .recover(handlers::rejection);

    warp::serve(routes).run((listen, port)).await;
}

fn with_state(
    state: Arc<Mutex<State>>,
) -> impl Filter<Extract = (Arc<Mutex<State>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn with_pg_client(
    client: Arc<Mutex<Client>>,
) -> impl Filter<Extract = (Arc<Mutex<Client>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || client.clone())
}

fn with_publisher(
    publisher: Arc<Mutex<Socket>>,
) -> impl Filter<Extract = (Arc<Mutex<Socket>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || publisher.clone())
}

fn with_debug(
    debug: Arc<bool>,
) -> impl Filter<Extract = (Arc<bool>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || debug.clone())
}
