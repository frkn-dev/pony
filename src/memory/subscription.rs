use chrono::{DateTime, Utc};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use std::ops::Deref;
use std::ops::DerefMut;

use crate::utils::get_uuid_last_octet_simple;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    pub id: uuid::Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub referred_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_deleted: bool,
}

impl Subscription {
    pub fn new(
        id: uuid::Uuid,
        ref_by: Option<String>,
        exp_at: Option<DateTime<Utc>>,
    ) -> Subscription {
        let now = Utc::now();
        Self {
            id: id,
            expires_at: exp_at,
            referred_by: ref_by,
            created_at: now,
            updated_at: now,
            is_deleted: false,
        }
    }
}

impl Default for Subscription {
    fn default() -> Self {
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4(),
            expires_at: None,
            referred_by: None,
            created_at: now,
            updated_at: now,
            is_deleted: false,
        }
    }
}

impl From<tokio_postgres::Row> for Subscription {
    fn from(row: tokio_postgres::Row) -> Self {
        let expires_at: Option<DateTime<Utc>> = row.get("expires_at");
        let created_at: DateTime<Utc> = row.get::<_, DateTime<Utc>>("created_at");
        let updated_at: DateTime<Utc> = row.get::<_, DateTime<Utc>>("updated_at");

        Self {
            id: row.get("id"),
            expires_at,
            referred_by: row.get("referred_by"),
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSubscription {
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStats {
    pub id: uuid::Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub days_remaining: i64,
    pub is_active: bool,
}

impl Subscription {
    pub fn stats(&self) -> SubscriptionStats {
        let now = Utc::now();
        let days_remaining = if let Some(expires_at) = self.expires_at {
            (expires_at - now).num_days()
        } else {
            99999
        };

        SubscriptionStats {
            id: self.id,
            expires_at: self.expires_at,
            days_remaining,
            is_active: days_remaining > 0 && !self.is_deleted,
        }
    }
}

pub trait Operations {
    fn extend(&mut self, days: i64);
    fn id(&self) -> uuid::Uuid;
    fn expires_at(&self) -> Option<DateTime<Utc>>;
    fn referral_code(&self) -> String;
    fn referred_by(&self) -> Option<String>;
    fn is_active(&self) -> bool;
    fn days_remaining(&self) -> Option<i64>;
    fn set_expires_at(&mut self, expires_at: DateTime<Utc>) -> Result<(), String>;
    fn mark_deleted(&mut self);
}

impl Operations for Subscription {
    fn extend(&mut self, days: i64) {
        if let Some(expires_at) = self.expires_at {
            let new_exp = expires_at + chrono::Duration::days(days);
            self.expires_at = Some(new_exp);
        };
        self.updated_at = Utc::now();
    }

    fn id(&self) -> uuid::Uuid {
        self.id
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    fn referral_code(&self) -> String {
        get_uuid_last_octet_simple(&self.id)
    }
    fn referred_by(&self) -> Option<String> {
        self.referred_by.clone()
    }

    fn is_active(&self) -> bool {
        !self.is_deleted && self.expires_at > Some(Utc::now())
    }

    fn days_remaining(&self) -> Option<i64> {
        let now = Utc::now();
        if let Some(expires_at) = self.expires_at {
            Some((expires_at - now).num_days())
        } else {
            None
        }
    }

    fn set_expires_at(&mut self, expires_at: DateTime<Utc>) -> Result<(), String> {
        if expires_at <= Utc::now() {
            return Err("Expiration date must be in the future".to_string());
        }
        self.expires_at = Some(expires_at);
        self.updated_at = Utc::now();
        Ok(())
    }

    fn mark_deleted(&mut self) {
        self.is_deleted = true;
        self.updated_at = Utc::now();
    }
}
