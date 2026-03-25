use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct Auth {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Api<T> {
    status: u16,
    message: String,
    pub response: T,
}
