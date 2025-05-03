use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    pub username: String,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
}

impl User {
    pub fn new(username: String) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            username,
            created_at: now,
            modified_at: now,
        }
    }
}
