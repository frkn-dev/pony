//! Premium slot inventory.
//!
//! Premium used to be nothing more than `subscriptions.scope_env` (a string
//! like `custompremium-prem02-4`) plus a `premium_token`: there was no stock to
//! count, no term to expire and nothing that stopped two live subscriptions
//! from claiming the same slot. A row in `premium_slots` is now the unit of
//! stock, and `assigned_sub` is its single owner.
//!
//! Two shapes of slot live in the same table:
//!
//! * `Pool` — a pre-provisioned address on a shared premium host (`prem02-4`,
//!   `prem-nl-01`). Its `env` is stable and known in advance, so it can be
//!   handed to any subscription.
//! * `Dedicated` — a whole machine for one client (`prem1…prem5`, plus the
//!   legacy `custompremium-*` envs). Its env is per-client and it is never part
//!   of the stock; it is registered so the admin panel shows one picture.
//!
//! Statuses are `free`, `reserved`, `assigned`, `disabled`. `reserved` matters:
//! an offer held for a waitlist request must not count as free stock and must
//! not be pickable, otherwise the same slot is offered twice.

use crate::memory::env::Env;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;

/// What kind of inventory a slot belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    /// Address on a shared premium host; part of the stock.
    Pool,
    /// Whole machine for one client; outside the stock.
    Dedicated,
}

impl SlotKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlotKind::Pool => "pool",
            SlotKind::Dedicated => "dedicated",
        }
    }

    /// Dedicated slots are never handed out of the pool stock.
    pub fn counts_towards_stock(&self) -> bool {
        matches!(self, SlotKind::Pool)
    }
}

impl fmt::Display for SlotKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SlotKind {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "pool" => Ok(SlotKind::Pool),
            "dedicated" => Ok(SlotKind::Dedicated),
            other => Err(crate::Error::Custom(format!("unknown slot kind: {other}"))),
        }
    }
}

/// Where a slot sits in its life cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotStatus {
    /// In stock, ready to be assigned.
    Free,
    /// Held for a waitlist request (`reserved_for`). Not stock, not pickable.
    Reserved,
    /// Owned by `assigned_sub`.
    Assigned,
    /// Out of stock on purpose: repair, degradation, or not provisioned yet.
    Disabled,
}

impl SlotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlotStatus::Free => "free",
            SlotStatus::Reserved => "reserved",
            SlotStatus::Assigned => "assigned",
            SlotStatus::Disabled => "disabled",
        }
    }

    /// Only `free` is stock: `reserved` is held for a request and `disabled`
    /// is out of rotation on purpose.
    pub fn is_stock(&self) -> bool {
        matches!(self, SlotStatus::Free)
    }
}

impl fmt::Display for SlotStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SlotStatus {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "free" => Ok(SlotStatus::Free),
            "reserved" => Ok(SlotStatus::Reserved),
            "assigned" => Ok(SlotStatus::Assigned),
            "disabled" => Ok(SlotStatus::Disabled),
            other => Err(crate::Error::Custom(format!(
                "unknown slot status: {other}"
            ))),
        }
    }
}

/// One unit of premium inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumSlot {
    /// Stable name from the inventory (`prem02-4`, `prem1`, `legacy-kyrgyzstan`).
    pub slot_key: String,
    /// `custompremium-prem02-4` — the same string as `subscriptions.scope_env`,
    /// so no join has to re-add the `custom` prefix. `None` for dedicated
    /// machines whose env is per-client.
    pub env: Option<Env>,
    pub kind: SlotKind,
    pub pool: String,
    pub slot_no: i16,
    pub address: Option<IpAddr>,
    /// H2 SNI; `None` means the slot serves AWG only.
    pub domain: Option<String>,
    pub country: Option<String>,
    pub status: SlotStatus,
    pub assigned_sub: Option<uuid::Uuid>,
    pub assigned_at: Option<DateTime<Utc>>,
    /// Waitlist request this slot is held for (`status = reserved`).
    pub reserved_for: Option<uuid::Uuid>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PremiumSlot {
    /// Whether the slot can serve traffic for a subscription: pool slot, in
    /// stock, not spoken for by an offer.
    pub fn is_assignable(&self) -> bool {
        self.kind == SlotKind::Pool && self.status.is_stock() && self.reserved_for.is_none()
    }
}

/// Life cycle of a waitlist request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    /// In the queue, waiting for a slot.
    Waiting,
    /// A slot is held for this request until `offer_expires_at`.
    Offered,
    /// The slot was handed over; the request is done.
    Converted,
    /// The user cancelled it.
    Cancelled,
    /// Dropped after too many missed offers.
    Expired,
}

impl RequestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequestStatus::Waiting => "waiting",
            RequestStatus::Offered => "offered",
            RequestStatus::Converted => "converted",
            RequestStatus::Cancelled => "cancelled",
            RequestStatus::Expired => "expired",
        }
    }

    /// Statuses that keep the request in the queue (and block a second one).
    pub fn is_active(&self) -> bool {
        matches!(self, RequestStatus::Waiting | RequestStatus::Offered)
    }
}

impl fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RequestStatus {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "waiting" => Ok(RequestStatus::Waiting),
            "offered" => Ok(RequestStatus::Offered),
            "converted" => Ok(RequestStatus::Converted),
            "cancelled" => Ok(RequestStatus::Cancelled),
            "expired" => Ok(RequestStatus::Expired),
            other => Err(crate::Error::Custom(format!(
                "unknown request status: {other}"
            ))),
        }
    }
}

/// A place in the premium waitlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumRequest {
    pub id: uuid::Uuid,
    pub subscription_id: uuid::Uuid,
    pub months: i16,
    pub contact: Option<String>,
    pub status: RequestStatus,
    /// Higher wins; paying clients and manual bumps go up.
    pub priority: i16,
    /// Missed offers. At `MAX_PREMIUM_OFFER_ATTEMPTS` the request dies.
    pub attempts: i16,
    /// Slot env currently held for this request.
    pub offered_env: Option<String>,
    pub offered_at: Option<DateTime<Utc>>,
    pub offer_expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PremiumRequest {
    /// Queue key: priority first, then arrival. `created_at` is bumped when a
    /// missed offer sends the request back, which is what puts it at the end of
    /// its priority group instead of leaving it in front of everyone.
    pub fn queue_key(&self) -> (i16, DateTime<Utc>) {
        (self.priority, self.created_at)
    }
}

/// Per-pool counters, so the admin panel can say "prem02: 0/5 free".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolStock {
    pub free: i64,
    pub reserved: i64,
    pub assigned: i64,
    pub disabled: i64,
    pub total: i64,
}

/// Aggregate stock. `free` is what a client sees as availability; dedicated
/// slots and reservations are tracked but never counted as free.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PremiumStock {
    pub free: i64,
    pub reserved: i64,
    pub assigned: i64,
    pub disabled: i64,
    pub total: i64,
    pub by_pool: BTreeMap<String, PoolStock>,
}

impl PremiumStock {
    /// Aggregate a slot list. Pure, so the counting rules are unit-tested
    /// without a database.
    pub fn from_slots<'a, I>(slots: I) -> Self
    where
        I: IntoIterator<Item = &'a PremiumSlot>,
    {
        let mut stock = PremiumStock::default();

        for slot in slots {
            // Dedicated machines are outside the stock by definition.
            if !slot.kind.counts_towards_stock() {
                continue;
            }

            stock.total += 1;
            let pool = stock.by_pool.entry(slot.pool.clone()).or_default();
            pool.total += 1;

            match slot.status {
                SlotStatus::Free if slot.reserved_for.is_none() => {
                    stock.free += 1;
                    pool.free += 1;
                }
                SlotStatus::Free | SlotStatus::Reserved => {
                    // A `free` slot pinned to a request is spoken for.
                    stock.reserved += 1;
                    pool.reserved += 1;
                }
                SlotStatus::Assigned => {
                    stock.assigned += 1;
                    pool.assigned += 1;
                }
                SlotStatus::Disabled => {
                    stock.disabled += 1;
                    pool.disabled += 1;
                }
            }
        }

        stock
    }

    /// `true` when a new subscription cannot be served right now.
    pub fn is_out_of_stock(&self) -> bool {
        self.free <= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn request(priority: i16, created_at: DateTime<Utc>) -> PremiumRequest {
        PremiumRequest {
            id: uuid::Uuid::new_v4(),
            subscription_id: uuid::Uuid::new_v4(),
            months: 1,
            contact: None,
            status: RequestStatus::Waiting,
            priority,
            attempts: 0,
            offered_env: None,
            offered_at: None,
            offer_expires_at: None,
            note: None,
            created_at,
            updated_at: created_at,
        }
    }

    fn slot(kind: SlotKind, status: SlotStatus, pool: &str, reserved: bool) -> PremiumSlot {
        PremiumSlot {
            slot_key: format!("{pool}-x"),
            env: Some(Env::Custom(format!("premium-{pool}-x"))),
            kind,
            pool: pool.to_string(),
            slot_no: 1,
            address: None,
            domain: None,
            country: Some("PL".to_string()),
            status,
            assigned_sub: if status == SlotStatus::Assigned {
                Some(uuid::Uuid::new_v4())
            } else {
                None
            },
            assigned_at: None,
            reserved_for: reserved.then(uuid::Uuid::new_v4),
            note: None,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        }
    }

    #[test]
    fn test_request_status_roundtrip_and_active() {
        for status in [
            RequestStatus::Waiting,
            RequestStatus::Offered,
            RequestStatus::Converted,
            RequestStatus::Cancelled,
            RequestStatus::Expired,
        ] {
            assert_eq!(status.as_str().parse::<RequestStatus>().unwrap(), status);
        }
        assert!("paused".parse::<RequestStatus>().is_err());

        assert!(RequestStatus::Waiting.is_active());
        assert!(RequestStatus::Offered.is_active());
        assert!(!RequestStatus::Converted.is_active());
        assert!(!RequestStatus::Cancelled.is_active());
        assert!(!RequestStatus::Expired.is_active());
    }

    #[test]
    fn test_queue_order_is_priority_then_arrival() {
        let earlier = Utc.timestamp_opt(1_000, 0).unwrap();
        let later = Utc.timestamp_opt(2_000, 0).unwrap();

        let mut paying_late = request(10, later);
        let mut free_early = request(0, earlier);

        // The queue sorts on (priority DESC, created_at ASC): a paying client who
        // arrived later still comes first.
        assert!(paying_late.queue_key().0 > free_early.queue_key().0);

        // Same priority: whoever arrived first goes first.
        free_early.priority = 0;
        paying_late.priority = 0;
        assert!(free_early.queue_key().1 < paying_late.queue_key().1);
    }

    #[test]
    fn test_status_roundtrip() {
        for status in [
            SlotStatus::Free,
            SlotStatus::Reserved,
            SlotStatus::Assigned,
            SlotStatus::Disabled,
        ] {
            assert_eq!(status.as_str().parse::<SlotStatus>().unwrap(), status);
        }
        assert!("busy".parse::<SlotStatus>().is_err());
    }

    #[test]
    fn test_kind_roundtrip() {
        for kind in [SlotKind::Pool, SlotKind::Dedicated] {
            assert_eq!(kind.as_str().parse::<SlotKind>().unwrap(), kind);
        }
        assert!("pod".parse::<SlotKind>().is_err());
    }

    #[test]
    fn test_stock_counts_free_only_unreserved_pool_slots() {
        let slots = vec![
            slot(SlotKind::Pool, SlotStatus::Free, "prem02", false),
            slot(SlotKind::Pool, SlotStatus::Free, "prem02", true), // pinned to an offer
            slot(SlotKind::Pool, SlotStatus::Reserved, "prem02", true),
            slot(SlotKind::Pool, SlotStatus::Assigned, "prem03", false),
            slot(SlotKind::Pool, SlotStatus::Disabled, "prem03", false),
            slot(SlotKind::Dedicated, SlotStatus::Disabled, "prem1", false),
        ];

        let stock = PremiumStock::from_slots(&slots);
        assert_eq!(stock.free, 1);
        assert_eq!(stock.reserved, 2);
        assert_eq!(stock.assigned, 1);
        assert_eq!(stock.disabled, 1);
        // Dedicated machine is not stock at all.
        assert_eq!(stock.total, 5);
        assert!(!stock.is_out_of_stock());

        let prem02 = stock.by_pool.get("prem02").unwrap();
        assert_eq!(prem02.free, 1);
        assert_eq!(prem02.reserved, 2);
        assert_eq!(prem02.total, 3);
        assert_eq!(stock.by_pool.get("prem03").unwrap().total, 2);
        assert!(stock.by_pool.get("prem1").is_none());
    }

    #[test]
    fn test_stock_out_when_only_reserved_left() {
        let slots = vec![
            slot(SlotKind::Pool, SlotStatus::Assigned, "prem02", false),
            slot(SlotKind::Pool, SlotStatus::Reserved, "prem02", true),
        ];
        let stock = PremiumStock::from_slots(&slots);
        assert_eq!(stock.free, 0);
        assert!(stock.is_out_of_stock());
    }

    #[test]
    fn test_assignable_requires_pool_free_and_unreserved() {
        assert!(slot(SlotKind::Pool, SlotStatus::Free, "prem02", false).is_assignable());
        assert!(!slot(SlotKind::Pool, SlotStatus::Free, "prem02", true).is_assignable());
        assert!(!slot(SlotKind::Pool, SlotStatus::Reserved, "prem02", true).is_assignable());
        assert!(!slot(SlotKind::Pool, SlotStatus::Assigned, "prem02", false).is_assignable());
        assert!(!slot(SlotKind::Dedicated, SlotStatus::Free, "prem1", false).is_assignable());
    }
}
