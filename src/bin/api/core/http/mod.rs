use pony::http::AuthError;
use pony::http::MethodError;
use warp::reject;
use warp::{http::StatusCode, Rejection, Reply};

mod filters;
pub mod handlers;
pub mod routes;

#[derive(Debug)]
struct JsonError(String);
impl reject::Reject for JsonError {}

pub async fn rejection(reject: Rejection) -> Result<impl Reply, Rejection> {
    if reject.find::<MethodError>().is_some() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": "Method Not Allowed"
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::METHOD_NOT_ALLOWED,
        ))
    } else if let Some(_) = reject.find::<AuthError>() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": "UNAUTHORIZED"
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::UNAUTHORIZED,
        ))
    } else if let Some(err) = reject.find::<JsonError>() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": err.0
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::BAD_REQUEST,
        ))
    } else if reject.is_not_found() {
        let error_response = warp::reply::json(&serde_json::json!({
            "error": "Not Found"
        }));
        Ok(warp::reply::with_status(
            error_response,
            StatusCode::NOT_FOUND,
        ))
    } else {
        Err(reject)
    }
}
