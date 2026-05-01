use serde::{Deserialize, Serialize};

use crate::Settings;

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct MtprotoSettings {
    pub port: u16,
    pub secret: String,
}

impl Settings for MtprotoSettings {
    fn validate(&self) -> crate::Result<()> {
        Ok(())
    }
}
