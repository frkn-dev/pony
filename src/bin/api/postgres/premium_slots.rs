//! Premium slot registry (`premium_slots`).
//!
//! The table is created by `deploy/migrations/0005_premium_slots.sql`; this
//! module only reads and writes rows. Stock is aggregated in Rust
//! ([`PremiumStock::from_slots`]) over a tiny table, so there is no second place
//! where the counting rules live.
//!
//! Assignment is the delicate part: "take any free pool slot" must not hand the
//! same slot to two subscriptions, and it must not touch a slot held for a
//! waitlist offer. The pick is a single `UPDATE … WHERE slot_key = (SELECT …
//! FOR UPDATE SKIP LOCKED)` inside one transaction, together with the
//! `subscriptions` update, so either both rows move or neither does.

use chrono::{DateTime, Utc};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use fcore::{Env, PremiumSlot, PremiumStock, Result, SlotKind, SlotStatus};

use super::pg::PgClientManager;

/// `host(address)` rather than `address::TEXT`: the text form of an INET carries
/// the netmask (`195.133.80.21/32`), which does not parse back into an `IpAddr`
/// and silently turned every slot address into `null` in the API.
const SLOT_COLUMNS: &str = "slot_key, env, kind, pool, slot_no, host(address) AS address, domain, \
     country, status, assigned_sub, assigned_at, reserved_for, note, created_at, updated_at";

/// Filter for the registry listing. Every field is optional; unset means "all".
#[derive(Debug, Clone, Default)]
pub struct SlotFilter {
    pub status: Option<SlotStatus>,
    pub pool: Option<String>,
    pub kind: Option<SlotKind>,
}

/// What an admin registers after provisioning a slot on the host.
#[derive(Debug, Clone)]
pub struct NewSlot {
    pub slot_key: String,
    /// `custompremium-prem02-4`; `None` for a dedicated machine.
    pub env: Option<Env>,
    pub kind: SlotKind,
    pub pool: String,
    pub slot_no: i16,
    pub address: Option<IpAddr>,
    pub domain: Option<String>,
    pub country: Option<String>,
    pub note: Option<String>,
}

/// Which slot the caller wants. `Free` lets the registry choose, the others
/// pin one.
#[derive(Debug, Clone)]
pub enum AssignTarget {
    /// Any free pool slot, optionally restricted to one pool.
    Free { pool: Option<String> },
    /// A specific slot by its `slot_key`.
    SlotKey(String),
    /// A specific slot by its env (`custompremium-prem02-4`).
    Env(Env),
}

/// Outcome of removing a slot from the registry.
#[derive(Debug, Clone)]
pub enum RemoveOutcome {
    Removed(PremiumSlot),
    NotFound,
    /// Refused: the slot still has an owner, so the row is the only record of it.
    Assigned(PremiumSlot),
}

/// Outcome of `disable` / `enable`.
#[derive(Debug, Clone)]
pub enum SlotUpdateOutcome {
    Updated(PremiumSlot),
    NotFound,
    /// Refused: an owned slot must be detached first.
    Assigned(PremiumSlot),
}

/// Result of an assignment attempt.
#[derive(Debug, Clone)]
pub enum AssignOutcome {
    /// Slot taken and subscription switched to it.
    Assigned { slot: PremiumSlot, token: String },
    /// No free pool slot left — the caller turns this into a waitlist request.
    OutOfStock,
    /// The requested slot is already owned or held.
    SlotBusy(String),
    /// The requested slot is not in the registry.
    UnknownSlot(String),
    /// The subscription already holds a slot (re-attach needs a detach first).
    AlreadyAssigned(String),
}

impl PgPremiumSlots {
    /// List slots matching the filter, ordered by pool and slot number.
    pub async fn list(&self, filter: &SlotFilter) -> Result<Vec<PremiumSlot>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = format!(
            "SELECT {SLOT_COLUMNS} FROM premium_slots
              WHERE ($1::TEXT IS NULL OR status = $1)
                AND ($2::TEXT IS NULL OR pool = $2)
                AND ($3::TEXT IS NULL OR kind = $3)
              ORDER BY kind, pool, slot_no"
        );

        let status = filter.status.map(|s| s.as_str().to_string());
        let kind = filter.kind.map(|k| k.as_str().to_string());
        let rows = client
            .query(&query, &[&status, &filter.pool, &kind])
            .await?;

        rows.iter().map(row_to_slot).collect()
    }

    pub async fn get(&self, slot_key: &str) -> Result<Option<PremiumSlot>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = format!("SELECT {SLOT_COLUMNS} FROM premium_slots WHERE slot_key = $1");
        match client.query_opt(&query, &[&slot_key]).await? {
            Some(row) => Ok(Some(row_to_slot(&row)?)),
            None => Ok(None),
        }
    }

    /// Aggregate stock. Cheap enough to read the whole table: it holds tens of
    /// rows, and one source of truth beats a hand-written `GROUP BY`.
    pub async fn stock(&self) -> Result<PremiumStock> {
        let slots = self
            .list(&SlotFilter {
                kind: Some(SlotKind::Pool),
                ..Default::default()
            })
            .await?;
        Ok(PremiumStock::from_slots(&slots))
    }

    /// Register (or update) a slot. Idempotent by `slot_key`, so re-running the
    /// provisioning checklist does not duplicate inventory.
    pub async fn register(&self, slot: &NewSlot) -> Result<PremiumSlot> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let env = slot.env.as_ref().map(|e| e.to_string());
        let address = slot.address.map(|a| a.to_string());

        let query = format!(
            "INSERT INTO premium_slots
                 (slot_key, env, kind, pool, slot_no, address, domain, country, note)
             VALUES ($1, $2, $3, $4, $5, $6::INET, $7, $8, $9)
             ON CONFLICT (slot_key) DO UPDATE
                SET env = EXCLUDED.env,
                    kind = EXCLUDED.kind,
                    pool = EXCLUDED.pool,
                    slot_no = EXCLUDED.slot_no,
                    address = EXCLUDED.address,
                    domain = EXCLUDED.domain,
                    country = EXCLUDED.country,
                    note = EXCLUDED.note,
                    updated_at = now()
             RETURNING {SLOT_COLUMNS}"
        );

        let row = client
            .query_one(
                &query,
                &[
                    &slot.slot_key,
                    &env,
                    &slot.kind.as_str(),
                    &slot.pool,
                    &slot.slot_no,
                    &address,
                    &slot.domain,
                    &slot.country,
                    &slot.note,
                ],
            )
            .await?;

        row_to_slot(&row)
    }

    /// Remove a slot from the registry. Refused while it is owned: the row is
    /// the only record of who holds the address.
    pub async fn remove(&self, slot_key: &str) -> Result<RemoveOutcome> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let query = format!(
            "DELETE FROM premium_slots
              WHERE slot_key = $1 AND status IN ('free', 'disabled', 'reserved')
             RETURNING {SLOT_COLUMNS}"
        );
        if let Some(row) = client.query_opt(&query, &[&slot_key]).await? {
            return Ok(RemoveOutcome::Removed(row_to_slot(&row)?));
        }

        let current = format!("SELECT {SLOT_COLUMNS} FROM premium_slots WHERE slot_key = $1");
        match client.query_opt(&current, &[&slot_key]).await? {
            Some(row) => Ok(RemoveOutcome::Assigned(row_to_slot(&row)?)),
            None => Ok(RemoveOutcome::NotFound),
        }
    }

    /// Take a slot out of rotation (`disabled = true`) or bring it back.
    /// A reserved slot may be disabled too (the offer is dropped by the caller).
    pub async fn set_disabled(&self, slot_key: &str, disabled: bool) -> Result<SlotUpdateOutcome> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let status = if disabled { "disabled" } else { "free" };
        let query = format!(
            "UPDATE premium_slots
                SET status = $2, reserved_for = NULL, updated_at = now()
              WHERE slot_key = $1 AND status <> 'assigned'
             RETURNING {SLOT_COLUMNS}"
        );

        if let Some(row) = client.query_opt(&query, &[&slot_key, &status]).await? {
            return Ok(SlotUpdateOutcome::Updated(row_to_slot(&row)?));
        }

        let current = format!("SELECT {SLOT_COLUMNS} FROM premium_slots WHERE slot_key = $1");
        match client.query_opt(&current, &[&slot_key]).await? {
            Some(row) => Ok(SlotUpdateOutcome::Assigned(row_to_slot(&row)?)),
            None => Ok(SlotUpdateOutcome::NotFound),
        }
    }

    /// Attach a slot to a subscription.
    ///
    /// One transaction: the slot becomes `assigned` and the subscription gets
    /// `scope_env` / `premium_token` / `premium_until`. A subscription that
    /// already holds a slot is reported as [`AssignOutcome::AlreadyAssigned`]
    /// instead of being silently moved.
    pub async fn assign(
        &self,
        subscription_id: uuid::Uuid,
        target: &AssignTarget,
        premium_until: Option<DateTime<Utc>>,
    ) -> Result<AssignOutcome> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let tx = client.transaction().await?;

        // One slot per subscription — checked inside the transaction so two
        // concurrent admins cannot each land a slot on the same account.
        let existing: Option<String> = tx
            .query_opt(
                "SELECT slot_key FROM premium_slots WHERE assigned_sub = $1 FOR UPDATE",
                &[&subscription_id],
            )
            .await?
            .map(|row| row.get::<_, String>(0));

        if let Some(slot_key) = existing {
            tx.rollback().await?;
            return Ok(AssignOutcome::AlreadyAssigned(slot_key));
        }

        let picked = match target {
            AssignTarget::Free { pool } => {
                tx.query_opt(
                    "UPDATE premium_slots
                        SET status = 'assigned', assigned_sub = $1, assigned_at = now(),
                            reserved_for = NULL, updated_at = now()
                      WHERE slot_key = (
                          SELECT slot_key FROM premium_slots
                           WHERE kind = 'pool'
                             AND status = 'free'
                             AND reserved_for IS NULL
                             AND ($2::TEXT IS NULL OR pool = $2)
                           ORDER BY pool, slot_no
                           FOR UPDATE SKIP LOCKED
                           LIMIT 1)
                     RETURNING slot_key",
                    &[&subscription_id, pool],
                )
                .await?
            }
            AssignTarget::SlotKey(slot_key) => {
                tx.query_opt(
                    "UPDATE premium_slots
                        SET status = 'assigned', assigned_sub = $1, assigned_at = now(),
                            reserved_for = NULL, updated_at = now()
                      WHERE slot_key = $2
                        AND kind = 'pool'
                        AND status IN ('free', 'reserved')
                     RETURNING slot_key",
                    &[&subscription_id, slot_key],
                )
                .await?
            }
            AssignTarget::Env(env) => {
                tx.query_opt(
                    "UPDATE premium_slots
                        SET status = 'assigned', assigned_sub = $1, assigned_at = now(),
                            reserved_for = NULL, updated_at = now()
                      WHERE env = $2
                        AND kind = 'pool'
                        AND status IN ('free', 'reserved')
                     RETURNING slot_key",
                    &[&subscription_id, &env.to_string()],
                )
                .await?
            }
        };

        let Some(picked) = picked else {
            // Why the pick came back empty decides the HTTP code: `out_of_stock`
            // for the flexible target, `slot_busy` vs `unknown_slot` otherwise.
            // Read the state inside the same transaction — taking the client
            // lock again here would deadlock.
            let taken: Option<String> = match target {
                AssignTarget::Free { .. } => None,
                AssignTarget::SlotKey(slot_key) => tx
                    .query_opt(
                        "SELECT status FROM premium_slots WHERE slot_key = $1",
                        &[&slot_key],
                    )
                    .await?
                    .map(|row| row.get::<_, String>(0)),
                AssignTarget::Env(env) => tx
                    .query_opt(
                        "SELECT status FROM premium_slots WHERE env = $1",
                        &[&env.to_string()],
                    )
                    .await?
                    .map(|row| row.get::<_, String>(0)),
            };

            tx.rollback().await?;

            return Ok(match (target, taken) {
                (AssignTarget::Free { .. }, _) => AssignOutcome::OutOfStock,
                (AssignTarget::SlotKey(slot_key), Some(_)) => {
                    AssignOutcome::SlotBusy(slot_key.clone())
                }
                (AssignTarget::SlotKey(slot_key), None) => {
                    AssignOutcome::UnknownSlot(slot_key.clone())
                }
                (AssignTarget::Env(env), Some(_)) => AssignOutcome::SlotBusy(env.to_string()),
                (AssignTarget::Env(env), None) => AssignOutcome::UnknownSlot(env.to_string()),
            });
        };

        let slot_key: String = picked.get(0);
        let slot_row = tx
            .query_one(
                &format!("SELECT {SLOT_COLUMNS} FROM premium_slots WHERE slot_key = $1"),
                &[&slot_key],
            )
            .await?;
        let slot = row_to_slot(&slot_row)?;

        let env = match &slot.env {
            Some(env) => env.to_string(),
            None => {
                tx.rollback().await?;
                return Err(fcore::Error::Custom(format!(
                    "slot {slot_key} has no env and cannot be assigned"
                )));
            }
        };

        // Keep the existing token when re-anchoring a slot, mint one otherwise.
        // The token that actually landed is what the caller reports back.
        let token = format!("prem_{}", uuid::Uuid::new_v4().simple());
        let token_row = tx
            .query_one(
                "UPDATE subscriptions
                    SET scope_env = $2,
                        premium_token = COALESCE(premium_token, $3),
                        premium_until = $4,
                        updated_at = now()
                  WHERE id = $1 AND NOT is_deleted
                 RETURNING premium_token",
                &[&subscription_id, &env, &token, &premium_until],
            )
            .await?;
        let token: String = token_row.get(0);

        tx.commit().await?;

        Ok(AssignOutcome::Assigned { slot, token })
    }

    /// Detach the slot from a subscription and clear its premium fields.
    /// Idempotent: returns `None` when the subscription holds no slot.
    pub async fn release(&self, subscription_id: uuid::Uuid) -> Result<Option<PremiumSlot>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let tx = client.transaction().await?;

        let row = tx
            .query_opt(
                &format!(
                    "UPDATE premium_slots
                        SET status = 'free', assigned_sub = NULL, assigned_at = NULL,
                            reserved_for = NULL, updated_at = now()
                      WHERE assigned_sub = $1
                     RETURNING {SLOT_COLUMNS}"
                ),
                &[&subscription_id],
            )
            .await
            .map_err(db_err)?;

        tx.execute(
            "UPDATE subscriptions
                SET scope_env = NULL, premium_token = NULL, premium_until = NULL,
                    updated_at = now()
              WHERE id = $1",
            &[&subscription_id],
        )
        .await
        .map_err(db_err)?;

        tx.commit().await?;

        match row {
            Some(row) => Ok(Some(row_to_slot(&row)?)),
            None => Ok(None),
        }
    }

    /// Push `premium_until` forward by `days`, starting from now when the slot
    /// has no term yet. Returns the new date, or `None` when the subscription
    /// does not exist.
    pub async fn extend(
        &self,
        subscription_id: uuid::Uuid,
        days: i64,
    ) -> Result<Option<DateTime<Utc>>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                "UPDATE subscriptions
                    SET premium_until = COALESCE(premium_until, now())
                                        + make_interval(days => $2::INT),
                        updated_at = now()
                  WHERE id = $1 AND NOT is_deleted
                 RETURNING premium_until",
                &[&subscription_id, &(days as i32)],
            )
            .await?;

        Ok(row.map(|row| row.get::<_, DateTime<Utc>>(0)))
    }
    /// `premium_until` for a batch of subscriptions, so the admin slot table can
    /// show the term next to the owner without a query per row.
    pub async fn premium_until_map(
        &self,
        subscription_ids: &[uuid::Uuid],
    ) -> Result<std::collections::HashMap<uuid::Uuid, Option<DateTime<Utc>>>> {
        if subscription_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                "SELECT id, premium_until FROM subscriptions WHERE id = ANY($1)",
                &[&subscription_ids],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get::<_, uuid::Uuid>(0),
                    row.get::<_, Option<DateTime<Utc>>>(1),
                )
            })
            .collect())
    }

    /// Look a slot up by its env string (`custompremium-prem02-4`), which is
    /// what the subscription carries.
    pub async fn find_by_env(&self, env: &str) -> Result<Option<PremiumSlot>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let query = format!("SELECT {SLOT_COLUMNS} FROM premium_slots WHERE env = $1");
        match client.query_opt(&query, &[&env]).await? {
            Some(row) => Ok(Some(row_to_slot(&row)?)),
            None => Ok(None),
        }
    }

    /// `subscriptions.premium_until`, which the in-memory cache does not carry.
    pub async fn premium_until_of(
        &self,
        subscription_id: uuid::Uuid,
    ) -> Result<Option<DateTime<Utc>>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let row = client
            .query_opt(
                "SELECT premium_until FROM subscriptions WHERE id = $1",
                &[&subscription_id],
            )
            .await?;
        Ok(row.and_then(|row| row.get::<_, Option<DateTime<Utc>>>(0)))
    }

    /// Subscriptions whose premium term has run out. The worker detaches them
    /// and frees their slots.
    pub async fn expired_premium(&self, limit: i64) -> Result<Vec<uuid::Uuid>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                "SELECT id FROM subscriptions
                  WHERE premium_until IS NOT NULL
                    AND premium_until < now()
                    AND scope_env IS NOT NULL
                    AND NOT is_deleted
                  ORDER BY premium_until
                  LIMIT $1
                  FOR UPDATE SKIP LOCKED",
                &[&limit],
            )
            .await?;

        Ok(rows.iter().map(|row| row.get::<_, uuid::Uuid>(0)).collect())
    }

    /// Slots held by a subscription that no longer deserves one: deleted, or
    /// expired on its own `expires_at`. Without this pass a slot would stay
    /// occupied forever by an account that cannot use it — `premium_until` is
    /// NULL (indefinite) for most of the existing premium subscriptions, so the
    /// term check alone would never fire for them.
    pub async fn slots_of_dead_subscriptions(&self, limit: i64) -> Result<Vec<uuid::Uuid>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                "SELECT ps.assigned_sub FROM premium_slots ps
                   JOIN subscriptions s ON s.id = ps.assigned_sub
                  WHERE ps.status = 'assigned'
                    AND (s.is_deleted
                         OR (s.expires_at IS NOT NULL AND s.expires_at < now()))
                  ORDER BY ps.assigned_at
                  LIMIT $1
                  FOR UPDATE OF ps SKIP LOCKED",
                &[&limit],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| row.get::<_, uuid::Uuid>(0))
            .collect())
    }
}

/// `tokio_postgres::Error` prints as "db error" and hides the server message, so
/// a failed UPDATE reached the api log as an opaque 500. Keep the SQLSTATE and
/// the message.
fn db_err(error: tokio_postgres::Error) -> fcore::Error {
    match error.as_db_error() {
        Some(db) => {
            fcore::Error::Custom(format!("postgres {}: {}", db.code().code(), db.message()))
        }
        None => fcore::Error::Custom(format!("postgres: {error}")),
    }

}

fn row_to_slot(row: &tokio_postgres::Row) -> Result<PremiumSlot> {
    let env: Option<String> = row.get("env");
    let kind: String = row.get("kind");
    let status: String = row.get("status");
    let address: Option<String> = row.get("address");

    Ok(PremiumSlot {
        slot_key: row.get("slot_key"),
        env: env
            .map(|value| Env::from_str(&value))
            .transpose()
            .map_err(|e| fcore::Error::Custom(format!("bad env in premium_slots: {e}")))?,
        kind: SlotKind::from_str(&kind)?,
        pool: row.get("pool"),
        slot_no: row.get("slot_no"),
        address: address.and_then(|value| value.parse::<IpAddr>().ok()),
        domain: row.get("domain"),
        country: row.get("country"),
        status: SlotStatus::from_str(&status)?,
        assigned_sub: row.get("assigned_sub"),
        assigned_at: row.get("assigned_at"),
        reserved_for: row.get("reserved_for"),
        note: row.get("note"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Server-side registry. Holds the shared Postgres client manager, like the
/// other `Pg*` storages.
pub struct PgPremiumSlots {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgPremiumSlots {
    pub fn new(manager: Arc<Mutex<PgClientManager>>) -> Self {
        Self { manager }
    }
}
