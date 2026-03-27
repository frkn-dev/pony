use pony::memory::key::Code;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Key {
    pub code: Code,
    pub email: Option<String>,
}
