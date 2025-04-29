use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::{stats::ConnStat, tag::Tag};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnBase {
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub online: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub proto: Option<Vec<Tag>>,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
}

impl ConnBase {
    pub fn new(password: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            uplink: Some(0),
            downlink: Some(0),
            online: Some(0),
            created_at: now,
            modified_at: now,
            proto: None,
            password,
            user_id: None,
        }
    }
}

impl From<Conn> for ConnBase {
    fn from(conn: Conn) -> Self {
        ConnBase {
            uplink: conn.uplink,
            downlink: conn.downlink,
            online: conn.online,
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            proto: conn.proto,
            password: conn.password,
            user_id: conn.user_id,
        }
    }
}

impl From<&Conn> for ConnBase {
    fn from(conn: &Conn) -> Self {
        ConnBase {
            uplink: conn.uplink,
            downlink: conn.downlink,
            online: conn.online,
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            proto: conn.proto.clone(),
            password: conn.password.clone(),
            user_id: conn.user_id,
        }
    }
}

impl fmt::Display for ConnBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ConnBase {{")?;
        writeln!(
            f,
            "  uplink: {},",
            self.uplink.map_or("None".to_string(), |v| v.to_string())
        )?;
        writeln!(
            f,
            "  downlink: {},",
            self.downlink.map_or("None".to_string(), |v| v.to_string())
        )?;
        writeln!(
            f,
            "  online: {},",
            self.online.map_or("None".to_string(), |v| v.to_string())
        )?;
        writeln!(f, "  created_at: {},", self.created_at)?;
        writeln!(f, "  modified_at: {},", self.modified_at)?;
        writeln!(f, "  proto: {:?},", self.proto.as_ref().unwrap_or(&vec![]))?;
        writeln!(
            f,
            "  password: {},",
            self.password
                .as_ref()
                .map_or("<none>".to_string(), |_| "<secret>".to_string())
        )?;
        writeln!(
            f,
            "  user_id: {},",
            self.user_id.map_or("None".to_string(), |id| id.to_string())
        )?;
        write!(f, "}}")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Conn {
    pub trial: bool,
    pub limit: i64,
    pub env: String,
    pub status: ConnStatus,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub online: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub proto: Option<Vec<Tag>>,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
}

impl fmt::Display for Conn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Connection {{\n")?;

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
        write!(f, "  modified_at: {},\n", self.modified_at)?;

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

impl Conn {
    pub fn new(trial: bool, limit: i64, env: String, password: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            trial,
            limit,
            env,
            status: ConnStatus::Active,
            uplink: Some(0),
            downlink: Some(0),
            online: Some(0),
            created_at: now,
            modified_at: now,
            proto: None,
            password: password,
            user_id: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConnStatus {
    Active,
    Expired,
}

impl fmt::Display for ConnStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let status = match self {
            ConnStatus::Active => "Active",
            ConnStatus::Expired => "Expired",
        };
        write!(f, "{}", status)
    }
}

pub trait ConnBaseOp {
    fn get_uplink(&self) -> Option<i64>;
    fn set_uplink(&mut self, v: i64);
    fn reset_uplink(&mut self);

    fn get_downlink(&self) -> Option<i64>;
    fn set_downlink(&mut self, v: i64);
    fn reset_downlink(&mut self);

    fn get_online(&self) -> Option<i64>;
    fn set_online(&mut self, v: i64);

    fn get_password(&self) -> Option<String>;
    fn set_password(&mut self, new_password: Option<String>);

    fn get_modified_at(&self) -> DateTime<Utc>;
    fn update_modified_at(&mut self);

    fn get_user_id(&self) -> Option<uuid::Uuid>;

    fn add_proto(&mut self, tag: Tag);
    fn remove_proto(&mut self, tag: Tag);

    fn as_conn_stat(&self) -> ConnStat;
}

pub trait ConnApiOp {
    fn get_trial(&self) -> bool;
    fn set_trial(&mut self, v: bool);

    fn get_limit(&self) -> i64;
    fn set_limit(&mut self, v: i64);

    fn get_status(&self) -> ConnStatus;
    fn set_status(&mut self, s: ConnStatus);

    fn get_env(&self) -> String;
}

impl ConnBaseOp for Conn {
    fn get_uplink(&self) -> Option<i64> {
        self.uplink
    }
    fn set_uplink(&mut self, v: i64) {
        self.uplink = Some(v);
    }
    fn reset_uplink(&mut self) {
        self.uplink = Some(0);
    }

    fn get_downlink(&self) -> Option<i64> {
        self.downlink
    }
    fn set_downlink(&mut self, v: i64) {
        self.downlink = Some(v);
    }
    fn reset_downlink(&mut self) {
        self.downlink = Some(0);
    }

    fn get_online(&self) -> Option<i64> {
        self.online
    }
    fn set_online(&mut self, v: i64) {
        self.online = Some(v);
    }

    fn get_modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }
    fn update_modified_at(&mut self) {
        self.modified_at = Utc::now();
    }

    fn get_user_id(&self) -> Option<uuid::Uuid> {
        self.user_id
    }

    fn add_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            if !proto.contains(&tag) {
                proto.push(tag);
            }
        } else {
            self.proto = Some(vec![tag]);
        }
    }
    fn remove_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            proto.retain(|p| *p != tag);
            if proto.is_empty() {
                self.proto = None;
            }
        }
    }
    fn get_password(&self) -> Option<String> {
        self.password.clone()
    }
    fn set_password(&mut self, new_password: Option<String>) {
        self.password = new_password;
    }

    fn as_conn_stat(&self) -> ConnStat {
        ConnStat {
            uplink: self.uplink.unwrap_or(0),
            downlink: self.downlink.unwrap_or(0),
            online: self.online.unwrap_or(0),
        }
    }
}

impl ConnApiOp for Conn {
    fn get_trial(&self) -> bool {
        self.trial
    }
    fn set_trial(&mut self, v: bool) {
        self.trial = v;
    }

    fn get_limit(&self) -> i64 {
        self.limit
    }
    fn set_limit(&mut self, v: i64) {
        self.limit = v;
    }

    fn get_status(&self) -> ConnStatus {
        self.status.clone()
    }
    fn set_status(&mut self, s: ConnStatus) {
        self.status = s;
    }

    fn get_env(&self) -> String {
        self.env.clone()
    }
}

impl ConnBaseOp for ConnBase {
    fn get_uplink(&self) -> Option<i64> {
        self.uplink
    }
    fn set_uplink(&mut self, v: i64) {
        self.uplink = Some(v);
    }
    fn reset_uplink(&mut self) {
        self.uplink = Some(0);
    }

    fn get_downlink(&self) -> Option<i64> {
        self.downlink
    }
    fn set_downlink(&mut self, v: i64) {
        self.downlink = Some(v);
    }
    fn reset_downlink(&mut self) {
        self.downlink = Some(0);
    }

    fn get_online(&self) -> Option<i64> {
        self.online
    }
    fn set_online(&mut self, v: i64) {
        self.online = Some(v);
    }

    fn get_modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }
    fn update_modified_at(&mut self) {
        self.modified_at = Utc::now();
    }

    fn get_user_id(&self) -> Option<uuid::Uuid> {
        self.user_id
    }

    fn add_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            if !proto.contains(&tag) {
                proto.push(tag);
            }
        } else {
            self.proto = Some(vec![tag]);
        }
    }
    fn remove_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            proto.retain(|p| *p != tag);
            if proto.is_empty() {
                self.proto = None;
            }
        }
    }

    fn get_password(&self) -> Option<String> {
        self.password.clone()
    }
    fn set_password(&mut self, new_password: Option<String>) {
        self.password = new_password;
    }

    fn as_conn_stat(&self) -> ConnStat {
        ConnStat {
            uplink: self.uplink.unwrap_or(0),
            downlink: self.downlink.unwrap_or(0),
            online: self.online.unwrap_or(0),
        }
    }
}
