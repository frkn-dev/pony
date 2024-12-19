use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::xray_op::Tag;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    Active,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub trial: bool,
    pub limit: i64,
    pub status: UserStatus,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub modified_at: Option<DateTime<Utc>>,
    pub proto: Option<Vec<Tag>>,
    pub password: Option<String>,
}

impl User {
    pub fn new(trial: bool, limit: i64, password: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            trial,
            limit,
            status: UserStatus::Active,
            uplink: None,
            downlink: None,
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

    pub fn has_proto_tag(&self, tag: Tag) -> bool {
        if let Some(proto_tags) = &self.proto {
            return proto_tags.contains(&tag);
        }
        false
    }
}
