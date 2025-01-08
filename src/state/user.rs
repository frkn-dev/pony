use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::{stats::UserStat, tag::Tag};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub trial: bool,
    pub limit: i64,
    pub env: String,
    pub status: UserStatus,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub online: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub modified_at: Option<DateTime<Utc>>,
    pub proto: Option<Vec<Tag>>,
    pub password: Option<String>,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "User {{\n")?;

        write!(f, "  trial: {},\n", self.trial)?;
        write!(f, "  env: {},\n", self.env)?;
        write!(f, "  limit: {},\n", self.limit)?;
        write!(f, "  status: {},\n", self.status)?;
        if let Some(uplink) = self.uplink {
            write!(f, "  uplink: {},\n", uplink)?;
        } else {
            write!(f, "  uplink: None,\n")?;
        }
        if let Some(downlink) = self.downlink {
            write!(f, "  downlink: {},\n", downlink)?;
        } else {
            write!(f, "  downlink: None,\n")?;
        }
        if let Some(online) = self.online {
            write!(f, "  online: {},\n", online)?;
        } else {
            write!(f, "  online: None,\n")?;
        }
        write!(f, "  created_at: {},\n", self.created_at)?;

        if let Some(modified_at) = &self.modified_at {
            write!(f, "  modified_at: {},\n", modified_at)?;
        } else {
            write!(f, "  modified_at: None,\n")?;
        }

        if let Some(proto) = &self.proto {
            write!(f, "  proto: {:?},\n", proto)?;
        } else {
            write!(f, "  proto: None,\n")?;
        }

        if let Some(_password) = &self.password {
            write!(f, "  password: <secret_password>,\n")?;
        } else {
            write!(f, "  password: None,\n")?;
        }

        write!(f, "}}")
    }
}

impl User {
    pub fn new(trial: bool, limit: i64, env: String, password: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            trial,
            limit,
            env,
            status: UserStatus::Active,
            uplink: Some(0),
            downlink: Some(0),
            online: Some(0),
            created_at: now,
            modified_at: None,
            proto: None,
            password: password,
        }
    }

    pub fn update_modified_at(&mut self) {
        self.modified_at = Some(Utc::now());
    }

    pub fn reset_downlink(&mut self) {
        self.downlink = Some(0);
    }

    pub fn reset_uplink(&mut self) {
        self.uplink = Some(0);
    }

    pub fn add_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            if !proto.contains(&tag) {
                proto.push(tag);
            }
        } else {
            self.proto = Some(vec![tag]);
        }
    }

    pub fn remove_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            proto.retain(|p| *p != tag);
            if proto.is_empty() {
                self.proto = None;
            }
        }
    }

    pub fn as_user_stat(&self) -> UserStat {
        UserStat {
            uplink: self.uplink.unwrap_or(0),
            downlink: self.downlink.unwrap_or(0),
            online: self.online.unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    Active,
    Expired,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let status = match self {
            UserStatus::Active => "Active",
            UserStatus::Expired => "Expired",
        };
        write!(f, "{}", status)
    }
}
