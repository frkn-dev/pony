use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    pub telegram_id: Option<u64>,
    pub username: String,
    pub env: String,
    pub limit: Option<i32>,
    pub password: Option<String>,
    pub created_at: NaiveDateTime,
    pub modified_at: NaiveDateTime,
    pub is_deleted: bool,
}

impl User {
    pub fn new(
        username: String,
        telegram_id: Option<u64>,
        env: &str,
        limit: Option<i32>,
        password: Option<String>,
    ) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            telegram_id,
            username,
            env: env.to_string(),
            limit: limit,
            password: password,
            created_at: now,
            modified_at: now,
            is_deleted: false,
        }
    }
}
