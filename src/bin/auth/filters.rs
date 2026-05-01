use fcore::ApiAccessConfig;
use warp::Filter;

#[cfg(feature = "email")]
use super::email::EmailStore;

#[cfg(feature = "email")]
pub fn with_store(
    store: EmailStore,
) -> impl Filter<Extract = (EmailStore,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || store.clone())
}

pub fn with_api_settings(
    api: ApiAccessConfig,
) -> impl Filter<Extract = (ApiAccessConfig,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || api.clone())
}
