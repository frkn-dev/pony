use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use std::fmt;

use serde::Deserialize;
use serde::Serialize;

use super::op::api::Operations as ApiOps;
use super::op::base::Operations as BasOps;
use super::proto::Proto;
use super::stat::Stat;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Conn {
    pub env: String,
    pub proto: Proto,
    pub stat: Stat,
    pub subscription_id: Option<uuid::Uuid>,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub expired_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
}

impl fmt::Display for Conn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Connection {{\n")?;

        if let Some(subscription_id) = self.subscription_id {
            write!(f, "  subscription_id: {},\n", subscription_id)?;
        } else {
            write!(f, "  subscription_id: None,\n")?;
        }
        write!(f, "  env: {},\n", self.env)?;
        write!(f, "  conn stat: {}\n", self.stat)?;
        write!(f, "  created_at: {},\n", self.created_at)?;
        write!(f, "  modified_at: {},\n", self.modified_at)?;
        write!(f, "  expired_at: {:?},\n", self.expired_at)?;
        write!(f, "  proto: {:?},\n", self.proto)?;
        write!(f, "  deleted: {}\n", self.is_deleted)?;
        write!(f, "}}")
    }
}

impl PartialEq for Conn {
    fn eq(&self, other: &Self) -> bool {
        self.get_subscription_id() == other.get_subscription_id()
            && self.get_proto() == other.get_proto()
            && self.get_deleted() == other.get_deleted()
            && self.get_env() == other.get_env()
            && self.get_deleted() == other.get_deleted()
    }
}

impl Conn {
    pub fn new(
        env: &str,
        subscription_id: Option<uuid::Uuid>,
        stat: Stat,
        proto: Proto,
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
            subscription_id: subscription_id,
            is_deleted: false,
        }
    }
}

#[derive(Serialize)]
pub struct ConnWithId {
    pub id: uuid::Uuid,
    #[serde(flatten)]
    pub conn: Conn,
}
