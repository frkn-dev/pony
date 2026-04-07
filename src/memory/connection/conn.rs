use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use std::fmt;

use serde::Deserialize;
use serde::Serialize;

use super::op::api::Operations as ApiOps;
use super::op::base::Operations as BasOps;
use super::proto::Proto;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Conn {
    pub env: String,
    pub proto: Proto,
    pub subscription_id: Option<uuid::Uuid>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
}

impl fmt::Display for Conn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Connection {{")?;

        if let Some(subscription_id) = self.subscription_id {
            writeln!(f, "  subscription_id: {},", subscription_id)?;
        } else {
            writeln!(f, "  subscription_id: None,")?;
        }
        writeln!(f, "  env: {},", self.env)?;
        writeln!(f, "  created_at: {},", self.created_at)?;
        writeln!(f, "  modified_at: {},", self.modified_at)?;
        writeln!(f, "  expires_at: {:?},", self.expires_at)?;
        writeln!(f, "  proto: {:?},", self.proto)?;
        writeln!(f, "  deleted: {}", self.is_deleted)?;
        writeln!(f, "}}")
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
        proto: Proto,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        let now = Utc::now();

        Self {
            env: env.to_string(),
            created_at: now,
            modified_at: now,
            expires_at,
            proto,
            subscription_id,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnPatch {
    pub env: Option<String>,
    pub proto: Option<Proto>,
    pub subscription_id: Option<uuid::Uuid>,
    pub created_at: Option<NaiveDateTime>,
    pub modified_at: Option<NaiveDateTime>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_deleted: Option<bool>,
}
