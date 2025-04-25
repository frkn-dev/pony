use warp::reject;

pub mod debug;
pub mod handlers;

#[cfg(feature = "api")]
pub mod api;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}
