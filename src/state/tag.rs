use serde::{Deserialize, Serialize};
use std::fmt;
use tokio_postgres::types::FromSql;
use tokio_postgres::types::ToSql;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, Copy, ToSql, FromSql)]
#[postgres(name = "proto", rename_all = "snake_case")]
pub enum Tag {
    #[serde(rename = "VlessXtls")]
    VlessXtls,
    #[serde(rename = "VlessGrpc")]
    VlessGrpc,
    #[serde(rename = "Vmess")]
    Vmess,
    #[serde(rename = "Shadowsocks")]
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
