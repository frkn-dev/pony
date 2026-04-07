use serde::Serialize;

#[derive(Serialize)]
pub struct Auth {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}
