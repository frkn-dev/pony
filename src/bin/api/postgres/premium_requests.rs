//! Premium waitlist (`premium_requests`).
//!
//! Premium stock is finite and currently zero, so "no slot left" is a normal
//! answer, not an error: a client leaves a request and gets served in order when
//! something frees up.
//!
//! Queue rules, and why they are what they are:
//!
//! * order is `priority DESC, created_at ASC` — paying clients and manual bumps
//!   jump the queue, everyone else is served in arrival order;
//! * a free slot is never handed out straight from stock while a request is
//!   waiting: [`PgPremiumRequests::auto_offer`] reserves it for the head of the
//!   queue first, so stock and waitlist cannot drift apart;
//! * an offer holds the slot as `reserved` for `offer_ttl_hours`. No answer means
//!   the request goes back to the **end** of its priority group (`created_at` is
//!   bumped) rather than dying on the first miss — people miss mail. After
//!   `max_attempts` misses it expires for good.
//!
//! All of this is transactional: reserving a slot and offering it to a request
//! happen together, or not at all.

use chrono::{DateTime, Utc};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use fcore::{PremiumRequest, RequestStatus, Result};

use super::pg::PgClientManager;

const REQUEST_COLUMNS: &str = "id, subscription_id, months, contact, status, priority, attempts, \
     offered_env, offered_at, offer_expires_at, note, created_at, updated_at";

/// Why a request could not be created.
#[derive(Debug, Clone)]
pub enum CreateOutcome {
    Created(PremiumRequest),
    /// The subscription already has a waiting or offered request.
    AlreadyWaiting(PremiumRequest),
}

/// Result of offering a slot.
#[derive(Debug, Clone)]
pub enum OfferOutcome {
    Offered(PremiumRequest),
    /// No such request, or it is not in a state that can be offered.
    NotActive,
    /// No free pool slot to offer.
    NoStock,
    /// The slot does not exist or is not free.
    SlotUnavailable(String),
}

/// Result of turning an offer into a real premium.
#[derive(Debug, Clone)]
pub enum ConvertOutcome {
    Converted {
        request: PremiumRequest,
        env: String,
        slot_key: String,
        token: String,
        premium_until: Option<DateTime<Utc>>,
    },
    NotActive,
    OutOfStock,
}

pub struct PgPremiumRequests {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgPremiumRequests {
    pub fn new(manager: Arc<Mutex<PgClientManager>>) -> Self {
        Self { manager }
    }

    /// Leave a request. One active request per subscription is enforced by
    /// `premium_requests_one_active`, so a repeat call returns the existing one
    /// instead of failing.
    pub async fn create(
        &self,
        subscription_id: uuid::Uuid,
        months: i16,
        contact: Option<String>,
        priority: i16,
    ) -> Result<CreateOutcome> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let existing = client
            .query_opt(
                &format!(
                    "SELECT {REQUEST_COLUMNS} FROM premium_requests
                      WHERE subscription_id = $1 AND status IN ('waiting', 'offered')"
                ),
                &[&subscription_id],
            )
            .await?;

        if let Some(row) = existing {
            return Ok(CreateOutcome::AlreadyWaiting(row_to_request(&row)?));
        }

        let row = client
            .query_one(
                &format!(
                    "INSERT INTO premium_requests (subscription_id, months, contact, priority)
                     VALUES ($1, $2, $3, $4)
                     RETURNING {REQUEST_COLUMNS}"
                ),
                &[&subscription_id, &months, &contact, &priority],
            )
            .await?;

        Ok(CreateOutcome::Created(row_to_request(&row)?))
    }

    pub async fn get(&self, id: uuid::Uuid) -> Result<Option<PremiumRequest>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let row = client
            .query_opt(
                &format!("SELECT {REQUEST_COLUMNS} FROM premium_requests WHERE id = $1"),
                &[&id],
            )
            .await?;
        row.as_ref().map(row_to_request).transpose()
    }

    /// The active request of a subscription, if any.
    pub async fn active_for(&self, subscription_id: uuid::Uuid) -> Result<Option<PremiumRequest>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let row = client
            .query_opt(
                &format!(
                    "SELECT {REQUEST_COLUMNS} FROM premium_requests
                      WHERE subscription_id = $1 AND status IN ('waiting', 'offered')"
                ),
                &[&subscription_id],
            )
            .await?;
        row.as_ref().map(row_to_request).transpose()
    }

    /// Admin queue listing, newest priority first.
    pub async fn list(&self, status: Option<RequestStatus>) -> Result<Vec<PremiumRequest>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let status = status.map(|s| s.as_str().to_string());
        let rows = client
            .query(
                &format!(
                    "SELECT {REQUEST_COLUMNS} FROM premium_requests
                      WHERE ($1::TEXT IS NULL OR status = $1)
                      ORDER BY priority DESC, created_at ASC"
                ),
                &[&status],
            )
            .await?;

        rows.iter().map(row_to_request).collect()
    }

    /// 1-based position in the queue among active requests, `None` when the
    /// request is finished. Only shown as "roughly N-th" to the user: offers and
    /// priority bumps move it.
    pub async fn position(&self, id: uuid::Uuid) -> Result<Option<i64>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                "SELECT (
                     SELECT count(*) FROM premium_requests r2
                      WHERE r2.status IN ('waiting', 'offered')
                        AND (r2.priority > r.priority
                             OR (r2.priority = r.priority AND r2.created_at < r.created_at))
                 ) + 1
                   FROM premium_requests r
                  WHERE r.id = $1 AND r.status IN ('waiting', 'offered')",
                &[&id],
            )
            .await?;

        Ok(row.map(|row| row.get::<_, i64>(0)))
    }

    /// Cancel a request. If a slot was held for it, the slot goes back to stock
    /// (and the next waiting request gets offered it by the worker).
    pub async fn cancel(&self, id: uuid::Uuid, subscription_id: uuid::Uuid) -> Result<bool> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;

        let row = tx
            .query_opt(
                "UPDATE premium_requests
                    SET status = 'cancelled', offered_env = NULL, offered_at = NULL,
                        offer_expires_at = NULL, updated_at = now()
                  WHERE id = $1 AND subscription_id = $2 AND status IN ('waiting', 'offered')
                 RETURNING id",
                &[&id, &subscription_id],
            )
            .await?;

        if row.is_none() {
            tx.rollback().await?;
            return Ok(false);
        }

        tx.execute(
            "UPDATE premium_slots
                SET status = 'free', reserved_for = NULL, updated_at = now()
              WHERE reserved_for = $1 AND status = 'reserved'",
            &[&id],
        )
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Manual priority bump (support: "this one paid first").
    pub async fn set_priority(&self, id: uuid::Uuid, priority: i16) -> Result<Option<PremiumRequest>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let row = client
            .query_opt(
                &format!(
                    "UPDATE premium_requests SET priority = $2, updated_at = now()
                      WHERE id = $1 AND status IN ('waiting', 'offered')
                     RETURNING {REQUEST_COLUMNS}"
                ),
                &[&id, &priority],
            )
            .await?;
        row.as_ref().map(row_to_request).transpose()
    }

    /// Offer a slot to one request. `slot_key = None` picks the lowest free pool
    /// slot. The slot becomes `reserved` for the request for `ttl_hours`.
    pub async fn offer(
        &self,
        id: uuid::Uuid,
        slot_key: Option<&str>,
        ttl_hours: i64,
    ) -> Result<OfferOutcome> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;

        let active = tx
            .query_opt(
                "SELECT status FROM premium_requests WHERE id = $1 FOR UPDATE",
                &[&id],
            )
            .await?
            .map(|row| row.get::<_, String>(0));

        match active.as_deref() {
            None | Some("converted") | Some("cancelled") | Some("expired") => {
                tx.rollback().await?;
                return Ok(OfferOutcome::NotActive);
            }
            _ => {}
        }

        // Release any slot previously held for this request before re-offering.
        tx.execute(
            "UPDATE premium_slots SET status = 'free', reserved_for = NULL, updated_at = now()
              WHERE reserved_for = $1 AND status = 'reserved'",
            &[&id],
        )
        .await?;

        let picked = match slot_key {
            Some(slot_key) => tx
                .query_opt(
                    "UPDATE premium_slots
                        SET status = 'reserved', reserved_for = $1, updated_at = now()
                      WHERE slot_key = $2 AND kind = 'pool' AND status = 'free'
                        AND reserved_for IS NULL
                     RETURNING env",
                    &[&id, &slot_key],
                )
                .await?,
            None => tx
                .query_opt(
                    "UPDATE premium_slots
                        SET status = 'reserved', reserved_for = $1, updated_at = now()
                      WHERE slot_key = (
                          SELECT slot_key FROM premium_slots
                           WHERE kind = 'pool' AND status = 'free' AND reserved_for IS NULL
                           ORDER BY pool, slot_no
                           FOR UPDATE SKIP LOCKED
                           LIMIT 1)
                     RETURNING env",
                    &[&id],
                )
                .await?,
        };

        let Some(picked) = picked else {
            tx.rollback().await?;
            return Ok(match slot_key {
                Some(slot_key) => OfferOutcome::SlotUnavailable(slot_key.to_string()),
                None => OfferOutcome::NoStock,
            });
        };

        let env: Option<String> = picked.get(0);

        let row = tx
            .query_one(
                &format!(
                    "UPDATE premium_requests
                        SET status = 'offered', offered_env = $2, offered_at = now(),
                            offer_expires_at = now() + make_interval(hours => $3::INT),
                            updated_at = now()
                      WHERE id = $1
                     RETURNING {REQUEST_COLUMNS}"
                ),
                &[&id, &env, &(ttl_hours as i32)],
            )
            .await?;

        tx.commit().await?;
        Ok(OfferOutcome::Offered(row_to_request(&row)?))
    }

    /// Turn an offer into a real premium: the reserved slot is assigned to the
    /// subscription and the request is marked converted, in one transaction.
    pub async fn convert(
        &self,
        id: uuid::Uuid,
        premium_until: Option<DateTime<Utc>>,
    ) -> Result<ConvertOutcome> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;

        let Some(request) = tx
            .query_opt(
                &format!(
                    "SELECT {REQUEST_COLUMNS} FROM premium_requests
                      WHERE id = $1 AND status IN ('waiting', 'offered') FOR UPDATE"
                ),
                &[&id],
            )
            .await?
        else {
            tx.rollback().await?;
            return Ok(ConvertOutcome::NotActive);
        };

        let request = row_to_request(&request)?;
        let subscription_id = request.subscription_id;

        // Prefer the slot held for this request; otherwise take any free one.
        let picked = tx
            .query_opt(
                "UPDATE premium_slots
                    SET status = 'assigned', assigned_sub = $1, assigned_at = now(),
                        reserved_for = NULL, updated_at = now()
                  WHERE slot_key = (
                      SELECT slot_key FROM premium_slots
                       WHERE kind = 'pool'
                         AND (reserved_for = $2 OR (status = 'free' AND reserved_for IS NULL))
                       -- `reserved_for IS NOT DISTINCT FROM $2`, not `= $2`:
                       -- a free slot has reserved_for = NULL, and in Postgres
                       -- `ORDER BY … DESC` puts NULLs FIRST, so `= $2` handed the
                       -- convert a different slot than the one offered.
                       ORDER BY (reserved_for IS NOT DISTINCT FROM $2) DESC, pool, slot_no
                       FOR UPDATE SKIP LOCKED
                       LIMIT 1)
                 RETURNING slot_key, env",
                &[&subscription_id, &id],
            )
            .await
            .map_err(db_err)?;

        let Some(picked) = picked else {
            tx.rollback().await?;
            return Ok(ConvertOutcome::OutOfStock);
        };

        let slot_key: String = picked.get(0);
        let env: Option<String> = picked.get(1);
        let Some(env) = env else {
            tx.rollback().await?;
            return Err(fcore::Error::Custom(format!(
                "slot {slot_key} has no env and cannot be assigned"
            )));
        };

        // One slot per subscription, same invariant as the direct assignment.
        let taken = tx
            .query_opt(
                "SELECT slot_key FROM premium_slots
                  WHERE assigned_sub = $1 AND slot_key <> $2 FOR UPDATE",
                &[&subscription_id, &slot_key],
            )
            .await?;
        if let Some(row) = taken {
            let existing: String = row.get(0);
            tx.rollback().await?;
            return Err(fcore::Error::Custom(format!(
                "subscription already holds slot {existing}"
            )));
        }

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
            .await
            .map_err(db_err)?;
        let token: String = token_row.get(0);

        let row = tx
            .query_one(
                &format!(
                    "UPDATE premium_requests
                        SET status = 'converted', offered_env = NULL, offered_at = NULL,
                            offer_expires_at = NULL, updated_at = now()
                      WHERE id = $1
                     RETURNING {REQUEST_COLUMNS}"
                ),
                &[&id],
            )
            .await?;

        tx.commit().await?;

        Ok(ConvertOutcome::Converted {
            request: row_to_request(&row)?,
            env,
            slot_key,
            token,
            premium_until,
        })
    }

    /// Admin refusal: the request dies and its held slot returns to stock.
    pub async fn decline(&self, id: uuid::Uuid, note: Option<String>) -> Result<bool> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;

        let row = tx
            .query_opt(
                "UPDATE premium_requests
                    SET status = 'cancelled', offered_env = NULL, offered_at = NULL,
                        offer_expires_at = NULL, note = COALESCE($2, note), updated_at = now()
                  WHERE id = $1 AND status IN ('waiting', 'offered')
                 RETURNING id",
                &[&id, &note],
            )
            .await?;

        if row.is_none() {
            tx.rollback().await?;
            return Ok(false);
        }

        tx.execute(
            "UPDATE premium_slots
                SET status = 'free', reserved_for = NULL, updated_at = now()
              WHERE reserved_for = $1 AND status = 'reserved'",
            &[&id],
        )
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Expire stale offers. A missed offer returns the request to the end of its
    /// priority group (`created_at` is bumped); after `max_attempts` misses it
    /// expires. Returns `(expired_requests, requeued_requests)`.
    pub async fn expire_offers(&self, max_attempts: i16) -> Result<(usize, usize)> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;

        let stale = tx
            .query(
                "SELECT id, attempts FROM premium_requests
                  WHERE status = 'offered' AND offer_expires_at IS NOT NULL
                    AND offer_expires_at < now()
                  FOR UPDATE SKIP LOCKED",
                &[],
            )
            .await?;

        let mut expired = 0usize;
        let mut requeued = 0usize;

        for row in stale {
            let id: uuid::Uuid = row.get(0);
            let attempts: i16 = row.get(1);

            // The held slot goes back to stock either way; the worker offers it
            // to the next request on its next pass.
            tx.execute(
                "UPDATE premium_slots
                    SET status = 'free', reserved_for = NULL, updated_at = now()
                  WHERE reserved_for = $1 AND status = 'reserved'",
                &[&id],
            )
            .await?;

            let next_attempts = attempts.saturating_add(1);
            if next_attempts >= max_attempts {
                tx.execute(
                    "UPDATE premium_requests
                        SET status = 'expired', attempts = $2, offered_env = NULL,
                            offered_at = NULL, offer_expires_at = NULL, updated_at = now()
                      WHERE id = $1",
                    &[&id, &next_attempts],
                )
                .await?;
                expired += 1;
            } else {
                // `created_at = now()` is the requeue: the request lands behind
                // everyone else with the same priority instead of keeping its
                // original place.
                tx.execute(
                    "UPDATE premium_requests
                        SET status = 'waiting', attempts = $2, offered_env = NULL,
                            offered_at = NULL, offer_expires_at = NULL,
                            created_at = now(), updated_at = now()
                      WHERE id = $1",
                    &[&id, &next_attempts],
                )
                .await?;
                requeued += 1;
            }
        }

        tx.commit().await?;
        Ok((expired, requeued))
    }

    /// Offer every free pool slot to the queue, in order. Called after a slot is
    /// released and on every worker tick, so stock never sits idle while someone
    /// is waiting. Returns the `(slot_key, request_id)` pairs offered.
    pub async fn auto_offer(&self, ttl_hours: i64) -> Result<Vec<(String, uuid::Uuid)>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;
        let tx = client.transaction().await?;

        let slots = tx
            .query(
                "SELECT slot_key FROM premium_slots
                  WHERE kind = 'pool' AND status = 'free' AND reserved_for IS NULL
                  ORDER BY pool, slot_no
                  FOR UPDATE SKIP LOCKED",
                &[],
            )
            .await?;

        let mut offered = Vec::new();

        for slot in slots {
            let slot_key: String = slot.get(0);

            let Some(request) = tx
                .query_opt(
                    "SELECT id FROM premium_requests
                      WHERE status = 'waiting'
                      ORDER BY priority DESC, created_at ASC
                      FOR UPDATE SKIP LOCKED
                      LIMIT 1",
                    &[],
                )
                .await?
            else {
                // Nobody waiting: leave the rest of the stock free.
                break;
            };

            let request_id: uuid::Uuid = request.get(0);

            let env: Option<String> = tx
                .query_one(
                    "UPDATE premium_slots
                        SET status = 'reserved', reserved_for = $1, updated_at = now()
                      WHERE slot_key = $2
                     RETURNING env",
                    &[&request_id, &slot_key],
                )
                .await?
                .get(0);

            tx.execute(
                "UPDATE premium_requests
                    SET status = 'offered', offered_env = $2, offered_at = now(),
                        offer_expires_at = now() + make_interval(hours => $3::INT),
                        updated_at = now()
                  WHERE id = $1",
                &[&request_id, &env, &(ttl_hours as i32)],
            )
            .await?;

            offered.push((slot_key, request_id));
        }

        tx.commit().await?;
        Ok(offered)
    }
}

/// `tokio_postgres::Error` prints as "db error" and hides the server message, so
/// a failing statement reached the api log and the HTTP client as an opaque 500.
/// Keep the SQLSTATE and the message.
fn db_err(error: tokio_postgres::Error) -> fcore::Error {
    match error.as_db_error() {
        Some(db) => fcore::Error::Custom(format!(
            "postgres {}: {}",
            db.code().code(),
            db.message()
        )),
        None => fcore::Error::Custom(format!("postgres: {error}")),
    }
}

fn row_to_request(row: &tokio_postgres::Row) -> Result<PremiumRequest> {
    let status: String = row.get("status");
    Ok(PremiumRequest {
        id: row.get("id"),
        subscription_id: row.get("subscription_id"),
        months: row.get("months"),
        contact: row.get("contact"),
        status: RequestStatus::from_str(&status)?,
        priority: row.get("priority"),
        attempts: row.get("attempts"),
        offered_env: row.get("offered_env"),
        offered_at: row.get("offered_at"),
        offer_expires_at: row.get("offer_expires_at"),
        note: row.get("note"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
