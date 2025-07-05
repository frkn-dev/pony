use chrono::NaiveDateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use crate::state::connection::conn::Conn;
use crate::state::connection::proto::Proto;
use crate::state::connection::stat::Stat as ConnectionStat;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Base {
    pub stat: ConnectionStat,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub proto: Proto,
    pub user_id: Option<uuid::Uuid>,
    pub is_deleted: bool,
}

impl Base {
    pub fn new(proto: Proto) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            stat: ConnectionStat::default(),
            created_at: now,
            modified_at: now,
            user_id: None,
            proto,
            is_deleted: false,
        }
    }
}

impl From<Conn> for Base {
    fn from(conn: Conn) -> Self {
        let conn_stat = ConnectionStat {
            online: conn.stat.online,
            uplink: conn.stat.uplink,
            downlink: conn.stat.downlink,
        };
        Base {
            stat: conn_stat,
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            proto: conn.proto,
            user_id: conn.user_id,
            is_deleted: conn.is_deleted,
        }
    }
}

impl From<&Conn> for Base {
    fn from(conn: &Conn) -> Self {
        let conn_stat = ConnectionStat {
            online: conn.stat.online,
            uplink: conn.stat.uplink,
            downlink: conn.stat.downlink,
        };
        Base {
            stat: conn_stat,
            created_at: conn.created_at,
            modified_at: conn.modified_at,
            proto: conn.proto.clone(),
            user_id: conn.user_id,
            is_deleted: conn.is_deleted,
        }
    }
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Connection Base {{")?;

        writeln!(f, " conn stat: {}", self.stat)?;
        writeln!(f, "  created_at: {},", self.created_at)?;
        writeln!(f, "  modified_at: {},", self.modified_at)?;
        writeln!(f, "  proto: {:?},", self.proto)?;
        writeln!(
            f,
            "  user_id: {},",
            self.user_id.map_or("None".to_string(), |id| id.to_string())
        )?;
        write!(f, "}}")
    }
}
