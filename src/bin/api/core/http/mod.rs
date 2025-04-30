use warp::reject;

mod filters;
pub mod handlers;
pub mod routes;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}
