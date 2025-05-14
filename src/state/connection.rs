use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use tokio_postgres::types::FromSql;
use tokio_postgres::types::ToSql;

use super::{stats::ConnStat, tag::Tag};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnBase {
    pub stat: ConnStat,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub proto: Tag,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
    pub is_deleted: bool,
}

impl ConnBase {
    pub fn new(proto: Tag, password: Option<String>) -> Self {
        let now = Utc::now();

        Self {
            stat: ConnStat::default(),
            created_at: now,
            modified_at: now,
            proto: proto,
            password,
            user_id: None,
            is_deleted: false,
        }
    }
}

impl From<Conn> for ConnBase {
    fn from(conn: Conn) -> Self {
        let conn_stat = ConnStat {
            online: conn.stat.online,
            uplink: conn.stat.uplink,
            downlink: conn.stat.downlink,
        };
        ConnBase {
            stat: conn_stat,
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            proto: conn.proto,
            password: conn.password,
            user_id: conn.user_id,
            is_deleted: conn.is_deleted,
        }
    }
}

impl From<&Conn> for ConnBase {
    fn from(conn: &Conn) -> Self {
        let conn_stat = ConnStat {
            online: conn.stat.online,
            uplink: conn.stat.uplink,
            downlink: conn.stat.downlink,
        };
        ConnBase {
            stat: conn_stat,
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            proto: conn.proto.clone(),
            password: conn.password.clone(),
            user_id: conn.user_id,
            is_deleted: conn.is_deleted,
        }
    }
}

impl fmt::Display for ConnBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ConnBase {{")?;

        writeln!(f, " conn stat: {}", self.stat)?;
        writeln!(f, "  created_at: {},", self.created_at)?;
        writeln!(f, "  modified_at: {},", self.modified_at)?;
        writeln!(f, "  proto: {:?},", self.proto)?;
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
    pub limit: i32,
    pub env: String,
    pub status: ConnStatus,
    pub stat: ConnStat,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub proto: Tag,
    pub password: Option<String>,
    pub user_id: Option<uuid::Uuid>,
    pub is_deleted: bool,
}

impl fmt::Display for Conn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Connection {{\n")?;

        if let Some(user_id) = self.user_id {
            write!(f, "  user_id: {},\n", user_id)?;
        } else {
            write!(f, "  user_id: None,\n")?;
        }
        write!(f, "  trial: {},\n", self.trial)?;
        write!(f, "  env: {},\n", self.env)?;
        write!(f, "  limit: {},\n", self.limit)?;
        write!(f, "  status: {},\n", self.status)?;
        write!(f, " conn stat: {}\n", self.stat)?;
        write!(f, "  created_at: {},\n", self.created_at)?;
        write!(f, "  modified_at: {},\n", self.modified_at)?;

        write!(f, "  proto: {:?},\n", self.proto)?;

        if let Some(_password) = &self.password {
            write!(f, "  password: <secret_password>,\n")?;
        } else {
            write!(f, "  password: None,\n")?;
        }
        write!(f, "deleted: {}\n", self.is_deleted)?;

        write!(f, "}}")
    }
}

impl Conn {
    pub fn new(
        trial: bool,
        limit: i32,
        env: &str,
        status: ConnStatus,
        password: Option<String>,
        user_id: Option<uuid::Uuid>,
        conn_stat: ConnStat,
        proto: Tag,
    ) -> Self {
        let now = Utc::now();

        Self {
            trial,
            limit,
            env: env.to_string(),
            status: status,
            stat: conn_stat,
            created_at: now,
            modified_at: now,
            proto: proto,
            password: password,
            user_id: user_id,
            is_deleted: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Copy, ToSql, FromSql)]
#[postgres(name = "conn_status", rename_all = "snake_case")]
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
    fn get_uplink(&self) -> i64;
    fn set_uplink(&mut self, v: i64);
    fn reset_uplink(&mut self);

    fn get_downlink(&self) -> i64;
    fn set_downlink(&mut self, v: i64);
    fn reset_downlink(&mut self);

    fn get_online(&self) -> i64;
    fn set_online(&mut self, v: i64);

    fn get_password(&self) -> Option<String>;
    fn set_password(&mut self, new_password: Option<String>);

    fn get_modified_at(&self) -> DateTime<Utc>;
    fn set_modified_at(&mut self);

    fn set_proto(&mut self, tag: Tag);
    fn get_proto(&self) -> Tag;

    fn as_conn_stat(&self) -> ConnStat;

    fn set_deleted(&mut self, v: bool);
    fn get_deleted(&self) -> bool;
}

pub trait ConnApiOp {
    fn get_trial(&self) -> bool;
    fn set_trial(&mut self, v: bool);

    fn get_limit(&self) -> i32;
    fn set_limit(&mut self, v: i32);

    fn get_status(&self) -> ConnStatus;
    fn set_status(&mut self, s: ConnStatus);

    fn get_user_id(&self) -> Option<uuid::Uuid>;
    fn set_user_id(&mut self, user_id: &uuid::Uuid);

    fn get_env(&self) -> String;
}

impl ConnBaseOp for Conn {
    fn get_uplink(&self) -> i64 {
        self.stat.uplink
    }
    fn set_uplink(&mut self, v: i64) {
        self.stat.uplink = v;
    }
    fn reset_uplink(&mut self) {
        self.stat.uplink = 0;
    }

    fn get_downlink(&self) -> i64 {
        self.stat.downlink
    }
    fn set_downlink(&mut self, v: i64) {
        self.stat.downlink = v;
    }
    fn reset_downlink(&mut self) {
        self.stat.downlink = 0;
    }

    fn get_online(&self) -> i64 {
        self.stat.online
    }
    fn set_online(&mut self, v: i64) {
        self.stat.online = v;
    }

    fn get_modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }
    fn set_modified_at(&mut self) {
        self.modified_at = Utc::now();
    }

    fn set_proto(&mut self, tag: Tag) {
        self.proto = tag
    }
    fn get_proto(&self) -> Tag {
        self.proto.clone()
    }

    fn get_password(&self) -> Option<String> {
        self.password.clone()
    }
    fn set_password(&mut self, new_password: Option<String>) {
        self.password = new_password;
    }

    fn set_deleted(&mut self, v: bool) {
        self.is_deleted = v;
    }
    fn get_deleted(&self) -> bool {
        self.is_deleted.clone()
    }

    fn as_conn_stat(&self) -> ConnStat {
        ConnStat {
            uplink: self.stat.uplink,
            downlink: self.stat.downlink,
            online: self.stat.online,
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

    fn get_limit(&self) -> i32 {
        self.limit
    }
    fn set_limit(&mut self, v: i32) {
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

    fn get_user_id(&self) -> Option<uuid::Uuid> {
        self.user_id.clone()
    }
    fn set_user_id(&mut self, user_id: &uuid::Uuid) {
        self.user_id = Some(*user_id);
    }
}

impl ConnBaseOp for ConnBase {
    fn get_uplink(&self) -> i64 {
        self.stat.uplink
    }
    fn set_uplink(&mut self, v: i64) {
        self.stat.uplink = v;
    }
    fn reset_uplink(&mut self) {
        self.stat.uplink = 0;
    }

    fn get_downlink(&self) -> i64 {
        self.stat.downlink
    }
    fn set_downlink(&mut self, v: i64) {
        self.stat.downlink = v;
    }
    fn reset_downlink(&mut self) {
        self.stat.downlink = 0;
    }

    fn get_online(&self) -> i64 {
        self.stat.online
    }
    fn set_online(&mut self, v: i64) {
        self.stat.online = v;
    }

    fn get_modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }
    fn set_modified_at(&mut self) {
        self.modified_at = Utc::now();
    }

    fn set_proto(&mut self, tag: Tag) {
        self.proto = tag
    }
    fn get_proto(&self) -> Tag {
        self.proto.clone()
    }

    fn get_password(&self) -> Option<String> {
        self.password.clone()
    }
    fn set_password(&mut self, new_password: Option<String>) {
        self.password = new_password;
    }

    fn as_conn_stat(&self) -> ConnStat {
        ConnStat {
            uplink: self.stat.uplink,
            downlink: self.stat.downlink,
            online: self.stat.online,
        }
    }

    fn set_deleted(&mut self, v: bool) {
        self.is_deleted = v;
    }
    fn get_deleted(&self) -> bool {
        self.is_deleted.clone()
    }
}
