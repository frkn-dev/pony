use serde::{Deserialize, Serialize};

use crate::Settings;

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct SecretEntry {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct MtprotoSettings {
    pub port: u16,
    pub secret: Vec<SecretEntry>,
}

impl Settings for MtprotoSettings {
    fn validate(&self) -> crate::Result<()> {
        Ok(())
    }
}
