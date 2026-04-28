use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;

#[derive(Hash, Eq, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Env {
    Production,
    Experimental,
    Dev,
    Ru,
    Wl,
}

impl std::fmt::Display for Env {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Env::Dev => write!(f, "dev"),
            Env::Ru => write!(f, "ru"),
            Env::Wl => write!(f, "wl"),
            Env::Experimental => write!(f, "experimental"),
            Env::Production => write!(f, "production"),
        }
    }
}

use std::str::FromStr;

impl FromStr for Env {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "experimental" | "exp" => Ok(Env::Experimental),
            "development" | "dev" => Ok(Env::Dev),
            "production" | "prod" => Ok(Env::Production),
            "ru" => Ok(Env::Ru),
            "wl" => Ok(Env::Wl),
            _ => Err(Error::Custom("Wrong Env string".into())),
        }
    }
}
impl From<&str> for Env {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or(Env::Dev)
    }
}

impl From<String> for Env {
    fn from(s: String) -> Self {
        Env::from(s.as_str())
    }
}
