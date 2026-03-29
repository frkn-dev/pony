use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct Auth {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Deserialize)]
pub struct Api<T> {
    pub status: u16,
    pub message: String,
    pub response: T,
}
