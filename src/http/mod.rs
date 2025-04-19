use warp::reject;

pub mod debug;

#[cfg(feature = "api")]
pub mod api;
#[cfg(feature = "api")]
pub mod handlers;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}
