use chrono::NaiveDateTime;
use chrono::Utc;

use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use tokio_postgres::types::FromSql;
use tokio_postgres::types::ToSql;

use crate::state::connection::proto::Proto;
use crate::state::connection::stat::Stat;

use crate::state::connection::op::api::Operations as ApiOps;
use crate::state::connection::op::base::Operations as BasOps;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Conn {
    pub trial: bool,
    pub limit: i32,
    pub env: String,
    pub proto: Proto,
    pub status: Status,
    pub stat: Stat,
    pub user_id: Option<uuid::Uuid>,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub is_deleted: bool,
    pub node_id: Option<uuid::Uuid>,
}

impl PartialEq for Conn {
    fn eq(&self, other: &Self) -> bool {
        self.get_user_id() == other.get_user_id()
            && self.get_proto() == other.get_proto()
            && self.get_trial() == other.get_trial()
            && self.get_limit() == other.get_limit()
            && self.get_status() == other.get_status()
            && self.get_deleted() == other.get_deleted()
            && self.get_env() == other.get_env()
            && self.get_deleted() == other.get_deleted()
    }
}

impl fmt::Display for Conn {
    // COMMENT(qezz): This feels like a Debug implementation. That can be either derived,
    // or the helper functions can be used, so one doesn't need to manually match the
    // indentation
    //
    // More details for manual implementation: https://doc.rust-lang.org/std/fmt/trait.Debug.html
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // COMMENT(qezz): there's `writeln!()`
        write!(f, "Connection {{\n")?;

        if let Some(user_id) = self.user_id {
            write!(f, "  user_id: {},\n", user_id)?;
        } else {
            write!(f, "  user_id: None,\n")?;
        }
        write!(
            f,
            "  node_id: {},\n",
            self.node_id.unwrap_or(uuid::Uuid::default())
        )?;
        write!(f, "  trial: {},\n", self.trial)?;
        write!(f, "  env: {},\n", self.env)?;
        write!(f, "  limit: {},\n", self.limit)?;
        write!(f, "  status: {},\n", self.status)?;
        write!(f, " conn stat: {}\n", self.stat)?;
        write!(f, "  created_at: {},\n", self.created_at)?;
        write!(f, "  modified_at: {},\n", self.modified_at)?;
        write!(f, "  proto: {:?},\n", self.proto)?;
        write!(f, "deleted: {}\n", self.is_deleted)?;
        write!(f, "}}")
    }
}

impl Conn {
    pub fn new(
        trial: bool,
        limit: i32,
        env: &str,
        status: Status,
        user_id: Option<uuid::Uuid>,
        stat: Stat,
        proto: Proto,
        node_id: Option<uuid::Uuid>,
    ) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            trial,
            limit,
            env: env.to_string(),
            status: status,
            stat: stat,
            created_at: now,
            modified_at: now,
            proto: proto,
            user_id: user_id,
            is_deleted: false,
            node_id: node_id,
        }
    }
}

#[derive(Serialize)]
pub struct ConnWithId {
    pub id: uuid::Uuid,
    #[serde(flatten)]
    pub conn: Conn,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Copy, ToSql, FromSql)]
#[postgres(name = "conn_status", rename_all = "snake_case")]
pub enum Status {
    Active,
    Expired,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let status = match self {
            Status::Active => "Active",
            Status::Expired => "Expired",
        };
        write!(f, "{}", status)
    }
}
