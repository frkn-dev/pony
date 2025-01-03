use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Tag {
    #[serde(rename = "vlessXtls")]
    VlessXtls,
    #[serde(rename = "vlessGrpc")]
    VlessGrpc,
    #[serde(rename = "vmess")]
    Vmess,
    #[serde(rename = "shadowsocks")]
    Shadowsocks,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::VlessXtls => write!(f, "VlessXtls"),
            Tag::VlessGrpc => write!(f, "VlessGrpc"),
            Tag::Vmess => write!(f, "Vmess"),
            Tag::Shadowsocks => write!(f, "Shadowsocks"),
        }
    }
}

impl std::str::FromStr for Tag {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "VlessXtls" => Ok(Tag::VlessXtls),
            "VlessGrpc" => Ok(Tag::VlessGrpc),
            "Vmess" => Ok(Tag::Vmess),
            "Shadowsocks" => Ok(Tag::Shadowsocks),
            _ => Err(()),
        }
    }
}
