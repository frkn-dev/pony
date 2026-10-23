//! Admin endpoints for the premium slot registry.
//!
//! Everything here is inventory management: list slots, register one after
//! provisioning, take one out of rotation, and answer "how much stock is left".
//! Assignment itself lives in [`super::admin`] (it has to touch the
//! subscription too) and detaching lives next to it.
//!
//! Responses are plain JSON with an `error` key for the refusals, so the
//! dashboard can branch on the code without parsing prose:
//!
//! * `404 slot_not_found` — no such `slot_key`;
//! * `409 slot_busy` — the slot still has an owner (detach first);
//! * `400 bad_slot` — the payload cannot describe a slot.

use std::net::IpAddr;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::json;
use warp::http::StatusCode;
use warp::Reply;

use fcore::{
    Env, PremiumStock, RequestStatus, SlotKind, SlotStatus, SubscriptionOperations,
    SubscriptionStorageOperations,
};

use super::admin::check_token;
use crate::postgres::premium_requests::{ConvertOutcome, OfferOutcome};
use crate::postgres::premium_slots::{
    AssignTarget, NewSlot, RemoveOutcome, SlotFilter, SlotUpdateOutcome,
};
use crate::sync::MemSync;

/// `GET /admin/api/premium/slots?status=free&pool=prem02&kind=pool`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlotListQuery {
    pub status: Option<String>,
    pub pool: Option<String>,
    pub kind: Option<String>,
}

/// `POST /admin/api/premium/slots`
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterSlotRequest {
    /// Inventory name, e.g. `prem02-4` or `prem1`.
    pub slot_key: String,
    /// Slot env as the API takes it (`premium-prem02-4`, `custom` prefix
    /// optional). Absent for a dedicated machine.
    pub env: Option<String>,
    /// `pool` (default) or `dedicated`.
    pub kind: Option<String>,
    pub pool: String,
    pub slot_no: i16,
    pub address: Option<String>,
    pub domain: Option<String>,
    pub country: Option<String>,
    pub note: Option<String>,
}

/// `GET /admin/api/premium/requests?status=waiting`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RequestListQuery {
    pub status: Option<String>,
}

/// `POST /admin/api/premium/requests/{id}/offer`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OfferRequestBody {
    /// Pin a slot; without it the lowest free pool slot is offered.
    #[serde(default)]
    pub slot_key: Option<String>,
    /// Override the hold time (hours). Default comes from the worker config.
    #[serde(default)]
    pub ttl_hours: Option<i64>,
}

/// `POST /admin/api/premium/requests/{id}/convert`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConvertRequestBody {
    /// Term in days; without it the premium is indefinite.
    #[serde(default)]
    pub days: Option<i64>,
}

/// `POST /admin/api/premium/requests/{id}/decline`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeclineRequestBody {
    #[serde(default)]
    pub note: Option<String>,
}

/// `POST /admin/api/premium/requests/{id}/priority`
#[derive(Debug, Clone, Deserialize)]
pub struct PriorityRequestBody {
    /// Higher wins. Support bumps this for clients who paid first; phase 3 will
    /// do it automatically when a premium SKU is confirmed.
    pub priority: i16,
}

fn reply_json(status: StatusCode, body: serde_json::Value) -> Box<dyn Reply + Send> {
    Box::new(warp::reply::with_status(warp::reply::json(&body), status))
}

/// `Some(reply)` when the caller must not proceed: admin disabled or the token
/// does not match.
fn authorize(
    admin_enabled: bool,
    admin_token: &str,
    auth_header: Option<String>,
) -> Option<Box<dyn Reply + Send>> {
    if !admin_enabled {
        return Some(reply_json(
            StatusCode::FORBIDDEN,
            json!({"error": "admin_disabled"}),
        ));
    }
    if !check_token(auth_header, admin_token) {
        return Some(reply_json(
            StatusCode::UNAUTHORIZED,
            json!({"error": "unauthorized"}),
        ));
    }
    None
}

fn parse_status(raw: Option<&String>) -> Result<Option<SlotStatus>, String> {
    match raw {
        None => Ok(None),
        Some(value) => value
            .parse::<SlotStatus>()
            .map(Some)
            .map_err(|_| format!("unknown status: {value}")),
    }
}

fn parse_kind(raw: Option<&String>) -> Result<Option<SlotKind>, String> {
    match raw {
        None => Ok(None),
        Some(value) => value
            .parse::<SlotKind>()
            .map(Some)
            .map_err(|_| format!("unknown kind: {value}")),
    }
}

/// `GET /admin/api/premium/slots`
pub async fn admin_api_premium_slots_handler<N, C, S>(
    query: SlotListQuery,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    let status = match parse_status(query.status.as_ref()) {
        Ok(status) => status,
        Err(error) => {
            return Ok(reply_json(
                StatusCode::BAD_REQUEST,
                json!({ "error": error }),
            ))
        }
    };
    let kind = match parse_kind(query.kind.as_ref()) {
        Ok(kind) => kind,
        Err(error) => {
            return Ok(reply_json(
                StatusCode::BAD_REQUEST,
                json!({ "error": error }),
            ))
        }
    };

    let filter = SlotFilter {
        status,
        pool: query.pool,
        kind,
    };

    match memory.db.premium_slots().list(&filter).await {
        Ok(slots) => {
            // The term belongs to the subscription, not to the slot, but the
            // admin table needs both side by side — one extra query for the
            // whole page instead of one per row.
            let owners: Vec<uuid::Uuid> = slots.iter().filter_map(|s| s.assigned_sub).collect();
            let until = memory
                .db
                .premium_slots()
                .premium_until_map(&owners)
                .await
                .unwrap_or_default();

            let slots: Vec<serde_json::Value> = slots
                .iter()
                .map(|slot| {
                    let mut value = json!(slot);
                    let premium_until = slot
                        .assigned_sub
                        .and_then(|id| until.get(&id).copied().flatten());
                    value["premium_until"] = json!(premium_until);
                    value
                })
                .collect();

            Ok(reply_json(
                StatusCode::OK,
                json!({"slots": slots, "count": slots.len()}),
            ))
        }
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `GET /admin/api/premium/stock`
pub async fn admin_api_premium_stock_handler<N, C, S>(
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    match memory.db.premium_slots().stock().await {
        Ok(stock) => {
            let mut body = json!(stock);
            body["out_of_stock"] = json!(stock.is_out_of_stock());
            Ok(reply_json(StatusCode::OK, body))
        }
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `POST /admin/api/premium/slots` — register or update a slot after
/// provisioning it on the host. Idempotent by `slot_key`.
pub async fn admin_api_premium_register_handler<N, C, S>(
    req: RegisterSlotRequest,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    let kind = match parse_kind(req.kind.as_ref()) {
        Ok(Some(kind)) => kind,
        Ok(None) => SlotKind::Pool,
        Err(error) => {
            return Ok(reply_json(
                StatusCode::BAD_REQUEST,
                json!({ "error": error }),
            ))
        }
    };

    // The env is accepted in either form (`premium-prem02-4` as the API docs
    // say, or the stored `custompremium-prem02-4`); `Env` normalises both.
    let env = match req.env.as_deref() {
        Some(raw) if !raw.is_empty() => match Env::parse_lenient(raw) {
            Ok(env) => Some(env),
            Err(e) => {
                return Ok(reply_json(
                    StatusCode::BAD_REQUEST,
                    json!({"error": format!("bad env: {e}")}),
                ))
            }
        },
        _ => None,
    };

    if kind == SlotKind::Pool && env.is_none() {
        return Ok(reply_json(
            StatusCode::BAD_REQUEST,
            json!({"error": "bad_slot", "detail": "pool slot needs env"}),
        ));
    }

    let address = match req.address.as_deref() {
        Some(raw) if !raw.is_empty() => match raw.parse::<IpAddr>() {
            Ok(address) => Some(address),
            Err(_) => {
                return Ok(reply_json(
                    StatusCode::BAD_REQUEST,
                    json!({"error": format!("bad address: {raw}")}),
                ))
            }
        },
        _ => None,
    };

    let slot = NewSlot {
        slot_key: req.slot_key,
        env,
        kind,
        pool: req.pool,
        slot_no: req.slot_no,
        address,
        domain: req.domain,
        country: req.country,
        note: req.note,
    };

    match memory.db.premium_slots().register(&slot).await {
        Ok(slot) => Ok(reply_json(StatusCode::OK, json!(slot))),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `DELETE /admin/api/premium/slots/{slot_key}`
pub async fn admin_api_premium_delete_handler<N, C, S>(
    slot_key: String,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    match memory.db.premium_slots().remove(&slot_key).await {
        Ok(RemoveOutcome::Removed(slot)) => Ok(reply_json(StatusCode::OK, json!(slot))),
        Ok(RemoveOutcome::NotFound) => Ok(reply_json(
            StatusCode::NOT_FOUND,
            json!({"error": "slot_not_found", "slot_key": slot_key}),
        )),
        Ok(RemoveOutcome::Assigned(slot)) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "slot_busy", "slot_key": slot.slot_key, "assigned_sub": slot.assigned_sub}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `POST /admin/api/premium/slots/{slot_key}/disable` and `…/enable`
pub async fn admin_api_premium_toggle_handler<N, C, S>(
    slot_key: String,
    disabled: bool,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    match memory
        .db
        .premium_slots()
        .set_disabled(&slot_key, disabled)
        .await
    {
        Ok(SlotUpdateOutcome::Updated(slot)) => Ok(reply_json(StatusCode::OK, json!(slot))),
        Ok(SlotUpdateOutcome::NotFound) => Ok(reply_json(
            StatusCode::NOT_FOUND,
            json!({"error": "slot_not_found", "slot_key": slot_key}),
        )),
        Ok(SlotUpdateOutcome::Assigned(slot)) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "slot_busy", "slot_key": slot.slot_key, "assigned_sub": slot.assigned_sub}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// Turn the optional `env` / `pool` of an admin request into a storage target.
/// An explicit env wins over a pool; neither means "any free pool slot".
pub fn assign_target(env: Option<&str>, pool: Option<&str>) -> Result<AssignTarget, String> {
    match (env, pool) {
        (Some(raw), _) if !raw.is_empty() => Env::parse_lenient(raw)
            .map(AssignTarget::Env)
            .map_err(|e| format!("bad env: {e}")),
        (_, Some(pool)) if !pool.is_empty() => Ok(AssignTarget::Free {
            pool: Some(pool.to_string()),
        }),
        _ => Ok(AssignTarget::Free { pool: None }),
    }
}

/// `premium_until` from an optional day count.
pub fn premium_until_from_days(
    days: Option<i64>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    match days {
        None => Ok(None),
        Some(days) if days <= 0 => Err("days must be positive".to_string()),
        Some(days) => {
            let now: chrono::DateTime<chrono::Utc> = SystemTime::now().into();
            Ok(Some(now + chrono::Duration::days(days)))
        }
    }
}

/// Stock summary for callers that want counters only (no addresses, no owner
/// ids). The client availability endpoint builds its own richer body, so this
/// stays available for future consumers and is unit-tested.
#[allow(dead_code)]
pub fn public_availability(stock: &PremiumStock) -> serde_json::Value {
    json!({
        "free": stock.free,
        "total": stock.total,
        "out_of_stock": stock.is_out_of_stock(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assign_target_prefers_env_then_pool() {
        match assign_target(Some("premium-prem02-4"), Some("prem03")).unwrap() {
            AssignTarget::Env(parsed) => {
                assert_eq!(parsed.to_string(), "custompremium-prem02-4")
            }
            other => panic!("expected Env target, got {other:?}"),
        }

        match assign_target(None, Some("prem02")).unwrap() {
            AssignTarget::Free { pool } => assert_eq!(pool.as_deref(), Some("prem02")),
            other => panic!("expected Free target, got {other:?}"),
        }

        match assign_target(None, None).unwrap() {
            AssignTarget::Free { pool } => assert!(pool.is_none()),
            other => panic!("expected Free target, got {other:?}"),
        }
    }

    #[test]
    fn test_premium_until_from_days() {
        assert!(premium_until_from_days(None).unwrap().is_none());
        assert!(premium_until_from_days(Some(0)).is_err());
        assert!(premium_until_from_days(Some(-5)).is_err());
        let one_month = premium_until_from_days(Some(30)).unwrap().unwrap();
        assert!(one_month > chrono::Utc::now());
    }

    #[test]
    fn test_public_availability_hides_addresses() {
        let mut stock = PremiumStock::default();
        stock.free = 0;
        stock.total = 12;
        let body = public_availability(&stock);
        assert_eq!(body["free"], 0);
        assert_eq!(body["total"], 12);
        assert_eq!(body["out_of_stock"], true);
        assert!(body.get("by_pool").is_none());
    }
}

/// `GET /admin/api/premium/requests` — the queue with positions.
pub async fn admin_api_premium_requests_handler<N, C, S>(
    query: RequestListQuery,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    let status = match query.status.as_deref() {
        None => None,
        Some(raw) => match raw.parse::<RequestStatus>() {
            Ok(status) => Some(status),
            Err(_) => {
                return Ok(reply_json(
                    StatusCode::BAD_REQUEST,
                    json!({"error": format!("unknown status: {raw}")}),
                ))
            }
        },
    };

    match memory.db.premium_requests().list(status).await {
        Ok(requests) => Ok(reply_json(
            StatusCode::OK,
            json!({"requests": requests, "count": requests.len()}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `POST /admin/api/premium/requests/{id}/offer` — hold a slot for one request.
pub async fn admin_api_premium_offer_handler<N, C, S>(
    request_id: uuid::Uuid,
    body: OfferRequestBody,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
    default_ttl_hours: i64,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    let ttl = body.ttl_hours.unwrap_or(default_ttl_hours).clamp(1, 720);

    match memory
        .db
        .premium_requests()
        .offer(request_id, body.slot_key.as_deref(), ttl)
        .await
    {
        Ok(OfferOutcome::Offered(request)) => Ok(reply_json(StatusCode::OK, json!(request))),
        Ok(OfferOutcome::NotActive) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "request_not_active", "request_id": request_id}),
        )),
        Ok(OfferOutcome::NoStock) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "out_of_stock"}),
        )),
        Ok(OfferOutcome::SlotUnavailable(slot_key)) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "slot_unavailable", "slot_key": slot_key}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `POST /admin/api/premium/requests/{id}/convert` — hand the slot over.
pub async fn admin_api_premium_convert_handler<N, C, S>(
    request_id: uuid::Uuid,
    body: ConvertRequestBody,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    let premium_until = match premium_until_from_days(body.days) {
        Ok(until) => until,
        Err(message) => {
            return Ok(reply_json(
                StatusCode::BAD_REQUEST,
                json!({"error": message}),
            ))
        }
    };

    match memory
        .db
        .premium_requests()
        .convert(request_id, premium_until)
        .await
    {
        Ok(ConvertOutcome::Converted {
            request,
            env,
            slot_key,
            token,
            premium_until,
        }) => {
            // Cache first, so the api serves the new env without waiting for the
            // periodic db sync.
            {
                let mut mem = memory.memory.write().await;
                if let Some(sub) = mem.subscriptions.find_by_id_mut(&request.subscription_id) {
                    if let Ok(parsed) = Env::parse_lenient(&env) {
                        sub.set_scope_env(parsed);
                    }
                    sub.set_premium_token(token.clone());
                }
            }

            crate::subscription_audit::log_premium_converted(
                request_id,
                request.subscription_id,
                &env,
                &slot_key,
            );

            Ok(reply_json(
                StatusCode::OK,
                json!({
                    "status": 200,
                    "request": request,
                    "env": env,
                    "slot_key": slot_key,
                    "premium_token": token,
                    "premium_until": premium_until,
                }),
            ))
        }
        Ok(ConvertOutcome::NotActive) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "request_not_active", "request_id": request_id}),
        )),
        Ok(ConvertOutcome::OutOfStock) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "out_of_stock"}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `POST /admin/api/premium/requests/{id}/decline`
pub async fn admin_api_premium_decline_handler<N, C, S>(
    request_id: uuid::Uuid,
    body: DeclineRequestBody,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    match memory
        .db
        .premium_requests()
        .decline(request_id, body.note)
        .await
    {
        Ok(true) => Ok(reply_json(
            StatusCode::OK,
            json!({"status": 200, "declined": true, "request_id": request_id}),
        )),
        Ok(false) => Ok(reply_json(
            StatusCode::CONFLICT,
            json!({"error": "request_not_active", "request_id": request_id}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}

/// `POST /admin/api/premium/requests/{id}/priority`
pub async fn admin_api_premium_priority_handler<N, C, S>(
    request_id: uuid::Uuid,
    body: PriorityRequestBody,
    memory: MemSync<N, C, S>,
    admin_enabled: bool,
    admin_token: String,
    auth_header: Option<String>,
) -> Result<Box<dyn Reply + Send>, warp::Rejection>
where
    N: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    if let Some(reply) = authorize(admin_enabled, &admin_token, auth_header) {
        return Ok(reply);
    }

    match memory
        .db
        .premium_requests()
        .set_priority(request_id, body.priority)
        .await
    {
        Ok(Some(request)) => Ok(reply_json(StatusCode::OK, json!(request))),
        Ok(None) => Ok(reply_json(
            StatusCode::NOT_FOUND,
            json!({"error": "request_not_active", "request_id": request_id}),
        )),
        Err(e) => Ok(reply_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e}")}),
        )),
    }
}
