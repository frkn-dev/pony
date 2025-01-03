use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;
use zmq::Socket;

use super::handlers::{self, node_register, NodesQueryParams};
use crate::state::{node::NodeRequest, state::State};

pub async fn run_api_server(state: Arc<Mutex<State>>, publisher: Arc<Mutex<Socket>>) {
    let user_route = warp::post()
        .and(warp::path("user"))
        .and(warp::body::json())
        .and(with_publisher(publisher.clone()))
        .and(with_state(state.clone()))
        .and_then(|user_req, publisher, state| handlers::user_request(user_req, publisher, state));

    let nodes_route = warp::get()
        .and(warp::path("nodes"))
        .and(warp::query::<NodesQueryParams>())
        .and(with_state(state.clone()))
        .and_then(handlers::get_nodes);

    let nodes_register_route = warp::post()
        .and(warp::path("node"))
        .and(warp::path("register"))
        .and(warp::body::json::<NodeRequest>())
        .and(with_state(state))
        .and(with_publisher(publisher))
        .and_then(node_register);

    let routes = user_route
        .or(nodes_route)
        .or(nodes_register_route)
        .recover(handlers::rejection);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

fn with_state(
    state: Arc<Mutex<State>>,
) -> impl Filter<Extract = (Arc<Mutex<State>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
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
