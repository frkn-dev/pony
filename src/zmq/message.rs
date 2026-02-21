use crate::memory::tag::ProtoTag;
use chrono::{DateTime, Utc};

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDes, Serialize as SerdeSer};
use std::fmt;

use crate::memory::connection::wireguard::Param as WgParam;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug))]
pub struct RkyvDateTime {
    timestamp: i64,
    nanos: u32,
}

impl From<DateTime<Utc>> for RkyvDateTime {
    fn from(dt: DateTime<Utc>) -> Self {
        Self {
            timestamp: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos(),
        }
    }
}

impl From<RkyvDateTime> for DateTime<Utc> {
    fn from(rkyv_dt: RkyvDateTime) -> Self {
        DateTime::from_timestamp(rkyv_dt.timestamp, rkyv_dt.nanos).expect("Invalid timestamp")
    }
}

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

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
#[archive(check_bytes)]
pub struct Message {
    pub conn_id: uuid::Uuid,
    pub action: Action,
    pub tag: ProtoTag,
    pub wg: Option<WgParam>,
    pub password: Option<String>,
    pub token: Option<uuid::Uuid>,
    pub expires_at: Option<RkyvDateTime>,
    pub subscription_id: Option<uuid::Uuid>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | {} | {} | {} | {}",
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
            },
            match &self.token {
                Some(t) => t.to_string(),
                None => "-".to_string(),
            },
            match &self.expires_at {
                Some(exp_at) => format!("{:?}", exp_at),
                None => "-".to_string(),
            }
        )
    }
}
