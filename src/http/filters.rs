use std::sync::Arc;
use warp::Filter;
use warp::Rejection;

use crate::http::AuthError;

pub fn auth(token: Arc<String>) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::<String>("authorization")
        .and_then(move |auth_header: String| {
            let token = token.clone();
            async move {
                if auth_header
                    .strip_prefix("Bearer ")
                    .map_or(false, |t| t == token.as_str())
                {
                    Ok(())
                } else {
                    Err(warp::reject::custom(AuthError("Unauthorized".to_string())))
                }
            }
        })
        .untuple_one()
}
