use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Trial {
    pub email: String,
    pub referred_by: Option<String>,
}

#[derive(Deserialize)]
pub struct Auth {
    pub addr: String,
    pub auth: uuid::Uuid,
    pub tx: u64,
}
