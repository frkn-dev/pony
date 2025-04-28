use warp::reject;

pub mod debug;
mod filters;
pub mod handlers;
mod routes;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}
