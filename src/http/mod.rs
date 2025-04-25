use warp::reject;

pub mod api;
pub mod debug;
pub mod handlers;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}
