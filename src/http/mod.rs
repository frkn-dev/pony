use serde::{Deserialize, Serialize};
use warp::reject;

pub mod debug;
pub mod requests;

#[derive(Debug)]
pub struct AuthError(pub String);
impl warp::reject::Reject for AuthError {}

#[derive(Debug)]
pub struct MethodError;
impl reject::Reject for MethodError {}

#[derive(Serialize, Debug, Deserialize)]
pub struct ResponseMessage<T> {
    pub status: u16,
    pub message: T,
}
