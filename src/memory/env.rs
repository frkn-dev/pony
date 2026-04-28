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
    Premium79634f2cb404,
    Premium17cd50b739ba,
    Premium1e3ee0f0dc12,
    Premium9196e6e9f257,
    Premium0629f01974d5,
    Premium52004488bcbe,
    Premiumb09f012e6d10,
    Premiumedd264cb559c,
}

impl std::fmt::Display for Env {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Env::Dev => write!(f, "dev"),
            Env::Ru => write!(f, "ru"),
            Env::Wl => write!(f, "wl"),
            Env::Experimental => write!(f, "experimental"),
            Env::Production => write!(f, "production"),
            Env::Premium79634f2cb404 => write!(f, "Premium79634f2cb404"),
            Env::Premium17cd50b739ba => write!(f, "Premium17cd50b739ba"),
            Env::Premium1e3ee0f0dc12 => write!(f, "Premium1e3ee0f0dc12"),
            Env::Premium9196e6e9f257 => write!(f, "Premium9196e6e9f257"),
            Env::Premium0629f01974d5 => write!(f, "Premium0629f01974d5"),
            Env::Premium52004488bcbe => write!(f, "Premium52004488bcbe"),
            Env::Premiumb09f012e6d10 => write!(f, "Premiumb09f012e6d10"),
            Env::Premiumedd264cb559c => write!(f, "Premiumedd264cb559c"),
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

            "premium17cd50b739ba" => Ok(Env::Premium17cd50b739ba),
            "premium79634f2cb404" => Ok(Env::Premium79634f2cb404),
            "premium1e3ee0f0dc12" => Ok(Env::Premium1e3ee0f0dc12),
            "premium9196e6e9f257" => Ok(Env::Premium9196e6e9f257),
            "premium0629f01974d5" => Ok(Env::Premium0629f01974d5),
            "premium52004488bcbe" => Ok(Env::Premium52004488bcbe),
            "premiumb09f012e6d10" => Ok(Env::Premiumb09f012e6d10),
            "premiumedd264cb559c" => Ok(Env::Premiumedd264cb559c),
            _ => Err(Error::Custom("Wrong Env string".into())),
        }
    }
}
impl From<&str> for Env {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or(Env::Experimental)
    }
}

impl From<String> for Env {
    fn from(s: String) -> Self {
        Env::from(s.as_str())
    }
}
