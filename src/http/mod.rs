use serde::{Deserialize, Serialize};
use warp::reject;
use warp::reject::Reject;

pub mod debug;
pub mod filters;
pub mod helpers;
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

impl Reject for Unauthorized {}

#[derive(Debug)]
pub struct IpParseError(pub std::net::AddrParseError);

impl Reject for IpParseError {}

#[derive(Debug)]
pub struct MyRejection(pub Box<dyn std::error::Error + Send + Sync>);

impl Reject for MyRejection {}
