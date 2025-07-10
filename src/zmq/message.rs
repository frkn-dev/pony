use crate::memory::tag::ProtoTag;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDes, Serialize as SerdeSer};
use std::fmt;

use crate::memory::connection::wireguard::Param as WgParam;

#[derive(Archive, Serialize, Deserialize, SerdeSer, SerdeDes, Debug, Clone)]
#[archive(check_bytes)]
pub enum Action {
    Create,
    Update,
    Delete,
    ResetStat,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Action::Create => write!(f, "Create"),
            Action::Delete => write!(f, "Delete"),
            Action::Update => write!(f, "Update"),
            Action::ResetStat => write!(f, "ResetStat"),
        }
    }
}

#[derive(Archive, Serialize, Deserialize, SerdeSer, SerdeDes, Clone, Debug)]
#[archive(check_bytes)]
pub struct Message {
    pub conn_id: UuidWrapper,
    pub action: Action,
    pub tag: ProtoTag,
    pub wg: Option<WgParam>,
    pub password: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | {} | {}",
            UuidWrapper::from(self.conn_id.clone()),
            self.action,
            self.tag,
            match &self.wg {
                Some(wg) => format!("{}", wg),
                None => "-".to_string(),
            },
            match &self.password {
                Some(pw) => pw.as_ref(),
                None => "-",
            }
        )
    }
}

#[derive(Archive, Serialize, Deserialize, SerdeSer, SerdeDes, Clone, Debug, PartialEq, Eq)]
pub struct UuidWrapper(pub [u8; 16]);

impl From<uuid::Uuid> for UuidWrapper {
    fn from(uuid: uuid::Uuid) -> Self {
        UuidWrapper(*uuid.as_bytes())
    }
}

impl From<UuidWrapper> for uuid::Uuid {
    fn from(wrapper: UuidWrapper) -> Self {
        uuid::Uuid::from_bytes(wrapper.0)
    }
}

impl fmt::Display for UuidWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let uuid = uuid::Uuid::from_bytes(self.0);
        write!(f, "{}", uuid)
    }
}
