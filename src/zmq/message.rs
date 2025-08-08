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
    pub conn_id: uuid::Uuid,
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
            self.conn_id.clone(),
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
