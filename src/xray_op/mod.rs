use serde::{Deserialize, Serialize};
use std::fmt;

pub mod client;
pub mod stats;
pub mod user_state;
pub mod users;
pub mod vless;
pub mod vmess;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum Tag {
    #[serde(rename = "vless")]
    Vless,
    #[serde(rename = "vmess")]
    Vmess,
    #[serde(rename = "shadowsocks")]
    Shadowsocks,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Vless => write!(f, "Vless"),
            Tag::Vmess => write!(f, "Vmess"),
            Tag::Shadowsocks => write!(f, "Shadowsocks"),
        }
    }
}
