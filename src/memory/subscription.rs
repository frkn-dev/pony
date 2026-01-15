use chrono::{DateTime, Utc};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use std::ops::Deref;
use std::ops::DerefMut;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    pub id: uuid::Uuid,
    pub expires_at: DateTime<Utc>,
    pub referral_code: Option<String>,
    pub referred_by: Option<uuid::Uuid>,
    pub referral_count: i32,
    pub referral_bonus_days: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_deleted: bool,
}

impl Default for Subscription {
    fn default() -> Self {
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4(),
            expires_at: now + chrono::Duration::days(30), // 30 дней по умолчанию
            referral_code: None,
            referred_by: None,
            referral_count: 0,
            referral_bonus_days: 0,
            created_at: now,
            updated_at: now,
            is_deleted: false,
        }
    }
}

impl From<tokio_postgres::Row> for Subscription {
    fn from(row: tokio_postgres::Row) -> Self {
        let expires_at: DateTime<Utc> = row.get::<_, DateTime<Utc>>("expires_at");
        let created_at: DateTime<Utc> = row.get::<_, DateTime<Utc>>("created_at");
        let updated_at: DateTime<Utc> = row.get::<_, DateTime<Utc>>("updated_at");

        Self {
            id: row.get("id"),
            expires_at,
            referral_code: row.get("referral_code"),
            referred_by: row.get("referred_by"),
            referral_count: row.get::<_, i32>("referral_count"),
            referral_bonus_days: row.get::<_, i32>("referral_bonus_days"),
            created_at,
            updated_at,
            is_deleted: row.get::<_, bool>("is_deleted"),
        }
    }
}

#[derive(
    Archive, PartialEq, Deserialize, Serialize, RkyvDeserialize, RkyvSerialize, Debug, Clone,
)]
#[archive(check_bytes)]
pub struct Subscriptions<S>(pub HashMap<uuid::Uuid, S>);

impl<S> Default for Subscriptions<S> {
    fn default() -> Self {
        Subscriptions(HashMap::new())
    }
}

impl<S: fmt::Display> fmt::Display for Subscriptions<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, sub) in &self.0 {
            writeln!(f, "{} => {}", id, sub)?;
        }
        Ok(())
    }
}

impl<S> Deref for Subscriptions<S> {
    type Target = HashMap<uuid::Uuid, S>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> DerefMut for Subscriptions<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSubscription {
    pub expires_at: DateTime<Utc>,
    pub referral_code: Option<String>,
    pub referred_by: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSubscription {
    pub expires_at: Option<DateTime<Utc>>,
    pub referral_count: Option<i32>,
    pub referral_bonus_days: Option<i32>,
    pub is_deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStats {
    pub id: uuid::Uuid,
    pub expires_at: DateTime<Utc>,
    pub referral_count: i32,
    pub referral_bonus_days: i32,
    pub days_remaining: i64,
    pub is_active: bool,
}

impl Subscription {
    pub fn stats(&self) -> SubscriptionStats {
        let now = Utc::now();
        let days_remaining = (self.expires_at - now).num_days();

        SubscriptionStats {
            id: self.id,
            expires_at: self.expires_at,
            referral_count: self.referral_count,
            referral_bonus_days: self.referral_bonus_days,
            days_remaining,
            is_active: days_remaining > 0 && !self.is_deleted,
        }
    }
}

pub trait Operations {
    fn extend(&mut self, days: i64);
    fn id(&self) -> uuid::Uuid;
    fn expires_at(&self) -> DateTime<Utc>;
    fn referral_code(&self) -> Option<String>;
    fn referred_by(&self) -> Option<uuid::Uuid>;
    fn is_active(&self) -> bool;
    fn days_remaining(&self) -> i64;
    fn set_expires_at(&mut self, expires_at: DateTime<Utc>) -> Result<(), String>;
    fn set_referral_code(&mut self, code: String) -> Result<(), String>;
    fn add_referral_bonus(&mut self, days: i32);
    fn increment_referral_count(&mut self);
    fn mark_deleted(&mut self);
}

impl Operations for Subscription {
    fn extend(&mut self, days: i64) {
        self.expires_at = self.expires_at + chrono::Duration::days(days);
        self.updated_at = Utc::now();
    }

    fn id(&self) -> uuid::Uuid {
        self.id
    }

    fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    fn referral_code(&self) -> Option<String> {
        self.referral_code.clone()
    }
    fn referred_by(&self) -> Option<uuid::Uuid> {
        self.referred_by.clone()
    }

    fn is_active(&self) -> bool {
        !self.is_deleted && self.expires_at > Utc::now()
    }

    fn days_remaining(&self) -> i64 {
        let now = Utc::now();
        (self.expires_at - now).num_days()
    }

    fn set_expires_at(&mut self, expires_at: DateTime<Utc>) -> Result<(), String> {
        if expires_at <= Utc::now() {
            return Err("Expiration date must be in the future".to_string());
        }
        self.expires_at = expires_at;
        self.updated_at = Utc::now();
        Ok(())
    }

    fn set_referral_code(&mut self, code: String) -> Result<(), String> {
        if code.len() != 12 || !code.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Referral code must be 12 hex characters".to_string());
        }
        self.referral_code = Some(code);
        self.updated_at = Utc::now();
        Ok(())
    }

    fn add_referral_bonus(&mut self, days: i32) {
        self.referral_bonus_days += days;
        self.expires_at += chrono::Duration::days(days as i64);
        self.updated_at = Utc::now();
    }

    fn increment_referral_count(&mut self) {
        self.referral_count += 1;
        self.updated_at = Utc::now();
    }

    fn mark_deleted(&mut self) {
        self.is_deleted = true;
        self.updated_at = Utc::now();
    }
}
