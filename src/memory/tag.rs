use rkyv::Archive;
use rkyv::{Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::fmt;
use tokio_postgres::types::FromSql;
use tokio_postgres::types::ToSql;

#[derive(
    Archive,
    Clone,
    Debug,
    RkyvDeserialize,
    RkyvSerialize,
    Deserialize,
    Serialize,
    PartialEq,
    Eq,
    Hash,
    Copy,
    ToSql,
    FromSql,
)]
#[archive_attr(derive(Clone, Debug))]
#[archive(check_bytes)]
#[postgres(name = "proto", rename_all = "snake_case")]
pub enum ProtoTag {
    #[serde(rename = "VlessXtls")]
    VlessXtls,
    #[serde(rename = "VlessGrpc")]
    VlessGrpc,
    #[serde(rename = "Vmess")]
    Vmess,
    #[serde(rename = "Shadowsocks")]
    Shadowsocks,
    #[serde(rename = "Wireguard")]
    Wireguard,
}

impl fmt::Display for ProtoTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtoTag::VlessXtls => write!(f, "VlessXtls"),
            ProtoTag::VlessGrpc => write!(f, "VlessGrpc"),
            ProtoTag::Vmess => write!(f, "Vmess"),
            ProtoTag::Shadowsocks => write!(f, "Shadowsocks"),
            ProtoTag::Wireguard => write!(f, "Wireguard"),
        }
    }
}

impl ProtoTag {
    pub fn is_wireguard(&self) -> bool {
        *self == ProtoTag::Wireguard
    }

    pub fn is_shadowsocks(&self) -> bool {
        *self == ProtoTag::Shadowsocks
    }
}

impl std::str::FromStr for ProtoTag {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "VlessXtls" => Ok(ProtoTag::VlessXtls),
            "VlessGrpc" => Ok(ProtoTag::VlessGrpc),
            "Vmess" => Ok(ProtoTag::Vmess),
            "Shadowsocks" => Ok(ProtoTag::Shadowsocks),
            "Wireguard" => Ok(ProtoTag::Wireguard),
            _ => Err(()),
        }
    }
}
