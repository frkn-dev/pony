use serde::Serialize;
use warp::http::StatusCode;
use warp::reply::Json;

use crate::Connection;
use crate::ConnectionStat;
use crate::Subscription;
use crate::Tag;

pub fn bad_request(msg: &str) -> warp::reply::WithStatus<Json> {
    let resp = ResponseMessage::<Option<uuid::Uuid>> {
        status: StatusCode::BAD_REQUEST.as_u16(),
        message: msg.to_string(),
        response: None,
    };
    warp::reply::with_status(warp::reply::json(&resp), StatusCode::BAD_REQUEST)
}

pub fn conflict(msg: &str) -> warp::reply::WithStatus<Json> {
    let resp = ResponseMessage::<Option<uuid::Uuid>> {
        status: StatusCode::CONFLICT.as_u16(),
        message: msg.to_string(),
        response: None,
    };
    warp::reply::with_status(warp::reply::json(&resp), StatusCode::CONFLICT)
}

pub fn internal_error(msg: &str) -> warp::reply::WithStatus<Json> {
    let resp = ResponseMessage::<Option<uuid::Uuid>> {
        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        message: msg.to_string(),
        response: None,
    };
    warp::reply::with_status(warp::reply::json(&resp), StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn not_found(msg: &str) -> warp::reply::WithStatus<Json> {
    let resp = ResponseMessage::<Option<uuid::Uuid>> {
        status: StatusCode::NOT_FOUND.as_u16(),
        message: msg.to_string(),
        response: None,
    };
    warp::reply::with_status(warp::reply::json(&resp), StatusCode::NOT_FOUND)
}

pub fn not_modified(msg: &str) -> warp::reply::WithStatus<Json> {
    let resp = ResponseMessage::<Option<uuid::Uuid>> {
        status: StatusCode::NOT_MODIFIED.as_u16(),
        message: msg.to_string(),
        response: None,
    };
    warp::reply::with_status(warp::reply::json(&resp), StatusCode::NOT_MODIFIED)
}

pub fn success_response(
    msg: String,
    id: Option<uuid::Uuid>,
    instance: Instance,
) -> warp::reply::WithStatus<Json> {
    let resp = ResponseMessage {
        status: StatusCode::OK.as_u16(),
        message: msg,
        response: Some(InstanceWithId { id, instance }),
    };
    warp::reply::with_status(warp::reply::json(&resp), StatusCode::OK)
}

#[derive(Serialize)]
struct ResponseMessage<T> {
    status: u16,
    message: String,
    response: T,
}

#[derive(Serialize)]
struct InstanceWithId {
    id: Option<uuid::Uuid>,
    instance: Instance,
}

#[derive(Serialize)]
pub enum Instance {
    Connection(Connection),
    Subscription(Subscription),
    Stat(Vec<(uuid::Uuid, ConnectionStat, Tag)>),
    Connections(Vec<(uuid::Uuid, Connection)>),
    None,
}
