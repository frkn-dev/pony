use serde::{Deserialize, Serialize};
use warp::reject;

pub mod debug;
pub mod filters;
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
    pub message: String,
    pub response: T,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct IdResponse {
    pub id: uuid::Uuid,
}

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}
