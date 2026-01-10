use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use super::op::api::Operations as ApiOps;
use super::op::base::Operations as BasOps;
use super::proto::Proto;
use super::stat::Stat;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Conn {
    pub env: String,
    pub proto: Proto,
    pub stat: Stat,
    pub user_id: Option<uuid::Uuid>,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub expired_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
    pub node_id: Option<uuid::Uuid>,
}

impl PartialEq for Conn {
    fn eq(&self, other: &Self) -> bool {
        self.get_user_id() == other.get_user_id()
            && self.get_proto() == other.get_proto()
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
        write!(f, "  env: {},\n", self.env)?;
        write!(f, " conn stat: {}\n", self.stat)?;
        write!(f, "  created_at: {},\n", self.created_at)?;
        write!(f, "  modified_at: {},\n", self.modified_at)?;
        write!(f, "  expired_at: {:?},\n", self.expired_at)?;
        write!(f, "  proto: {:?},\n", self.proto)?;
        write!(f, "deleted: {}\n", self.is_deleted)?;
        write!(f, "}}")
    }
}

impl Conn {
    pub fn new(
        env: &str,
        user_id: Option<uuid::Uuid>,
        stat: Stat,
        proto: Proto,
        node_id: Option<uuid::Uuid>,
        expired_at: Option<DateTime<Utc>>,
    ) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            env: env.to_string(),
            stat: stat,
            created_at: now,
            modified_at: now,
            expired_at: expired_at,
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
