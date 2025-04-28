use serde::{Deserialize, Serialize};

pub mod debug;

#[derive(Serialize, Debug, Deserialize)]
pub struct ResponseMessage<T> {
    pub status: u16,
    pub message: T,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserRequest {
    pub username: String,
}
