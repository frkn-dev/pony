use serde::Deserialize;
use serde::Serialize;

use super::super::tag::Tag;
use super::wireguard::Param as WgParam;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Proto {
    Wireguard { param: WgParam, node_id: uuid::Uuid },
    Shadowsocks { password: String },
    Xray(Tag),
}

impl Proto {
    pub fn proto(&self) -> Tag {
        match self {
            Proto::Wireguard { .. } => Tag::Wireguard,
            Proto::Shadowsocks { .. } => Tag::Shadowsocks,
            Proto::Xray(tag) => *tag,
        }
    }

    pub fn new_wg(param: &WgParam, node_id: &uuid::Uuid) -> Self {
        Proto::Wireguard {
            param: param.clone(),
            node_id: *node_id,
        }
    }

    pub fn new_ss(password: &str) -> Self {
        Proto::Shadowsocks {
            password: password.to_string(),
        }
    }

    pub fn new_xray(tag: &Tag) -> Self {
        Proto::Xray(*tag)
    }

    pub fn is_xray(&self) -> bool {
        matches!(self, Proto::Xray(_))
    }

    pub fn is_wireguard(&self) -> bool {
        matches!(self, Proto::Wireguard { .. })
    }

    pub fn is_shadowsocks(&self) -> bool {
        matches!(self, Proto::Shadowsocks { .. })
    }
}
