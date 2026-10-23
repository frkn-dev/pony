use chrono::{DateTime, Utc};
use tracing::Instrument;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use uuid::Uuid;
use warp::{http::StatusCode, Rejection, Reply};

use super::super::super::{
    subscription_audit,
    sync::{
        tasks::{SyncOp, DELETE_REASON_MANUAL},
        MemSync,
    },
};
use super::admin::AdminSubscriptionTraffic;
use super::subscription::build_subscription_traffic;
use crate::postgres::premium_requests::{ConvertOutcome, CreateOutcome};

use fcore::{
    http::helpers as http, Connection, ConnectionApiOperations, ConnectionBaseOperations,
    ConnectionStorageApiOperations, ConnectionStorageBaseOperations, Env, IpAddrMask,
    MetricStorage, NodeStorageOperations, Proto, Status, Subscription, SubscriptionOperations,
    SubscriptionStorageOperations, Tag, Topic, WgKeys, WgParam,
};

#[derive(Debug, Deserialize)]
pub struct PremiumChildCreateRequest {
    pub days: Option<i64>,
    pub limit_bytes: Option<i64>,
}

impl PremiumChildCreateRequest {
    pub fn validate(&self) -> Result<(), String> {
        match self.days {
            Some(days) if days > 0 => Ok(()),
            Some(_) => Err("days must be greater than 0".to_string()),
            None => Err("days is required".to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PremiumChildUpdateRequest {
    pub days: Option<i64>,
    pub limit_bytes: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PremiumConnectionCreateRequest {
    pub env: Env,
    pub proto: Tag,
    pub days: Option<u16>,
}

#[derive(Debug, Serialize)]
pub struct PremiumChild {
    pub id: Uuid,
    pub refer_code: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub limit_bytes: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PremiumConnection {
    pub id: Uuid,
    pub env: String,
    pub proto: String,
    pub subscription_id: Uuid,
    pub is_deleted: bool,
    pub traffic: AdminSubscriptionTraffic,
}

#[derive(Debug, Serialize)]
pub struct PremiumState {
    pub children_count: usize,
    pub active_children: usize,
    pub connections_count: usize,
    pub total_traffic: AdminSubscriptionTraffic,
    /// Premium env the account is anchored to (`custompremium-prem02-4`).
    pub env: Option<String>,
    /// Term end; `null` means indefinite (the pre-registry behaviour).
    pub premium_until: Option<DateTime<Utc>>,
    /// Slot the account sits on, when it is in the registry.
    pub slot: Option<PremiumSlotInfo>,
}

/// Slot identity as a client may see it: pool and number plus the address it
/// connects to. No owner ids, no internal notes.
#[derive(Debug, Serialize)]
pub struct PremiumSlotInfo {
    pub pool: String,
    pub slot_no: i16,
    pub domain: Option<String>,
    pub address: Option<String>,
    pub country: Option<String>,
}

/// `GET /premium/availability` — counters only, no addresses.
#[derive(Debug, Serialize)]
pub struct PremiumAvailability {
    pub free: i64,
    pub total: i64,
    pub out_of_stock: bool,
    /// The account already holds a premium slot.
    pub has_premium: bool,
    /// Term of that premium; `null` means indefinite.
    pub premium_until: Option<DateTime<Utc>>,
    /// Slot the account sits on, when it is in the registry.
    pub slot: Option<PremiumSlotInfo>,
    /// Position in the queue (1-based) when the account is waiting.
    pub waitlist_position: Option<i64>,
    /// `true` when a request can be left (stock is zero and none is active yet).
    pub can_request: bool,
    /// The active request, when there is one.
    pub request: Option<PremiumRequestView>,
}

/// What the client sees about its own request.
#[derive(Debug, Serialize)]
pub struct PremiumRequestView {
    pub id: Uuid,
    pub status: String,
    pub months: i16,
    pub position: Option<i64>,
    pub offered_env: Option<String>,
    pub offer_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// `?subscription_id=…` on the client-facing premium endpoints. Auth is the
/// service token, so the id is what says whose request it is.
#[derive(Debug, Clone, Deserialize)]
pub struct PremiumSubscriptionQuery {
    pub subscription_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct PremiumRequestCreateBody {
    pub subscription_id: Uuid,
    #[serde(default)]
    pub months: Option<i16>,
    #[serde(default)]
    pub contact: Option<String>,
}

fn ensure_scope<S: SubscriptionOperations>(parent: &S, env: &Env) -> Result<(), Rejection> {
    if let Some(scope) = parent.scope_env() {
        if scope != env {
            return Err(warp::reject::custom(fcore::http::AuthError(
                "Env out of scope".to_string(),
            )));
        }
    }
    Ok(())
}

fn ensure_child_of<S: SubscriptionOperations>(parent: &S, child: &S) -> Result<(), Rejection> {
    if child.parent_id() != Some(parent.id()) {
        return Err(warp::reject::custom(fcore::http::AuthError(
            "Forbidden".to_string(),
        )));
    }
    Ok(())
}

pub async fn premium_state_handler<N, C, S>(
    parent: S,
    memory: MemSync<N, C, S>,
    metrics: Arc<MetricStorage>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let mem = memory.memory.read().await;
    let children = mem.subscriptions.find_by_parent_id(&parent.id());

    let mut total_uplink = 0u64;
    let mut total_downlink = 0u64;
    let mut connections_count = 0usize;

    for child in &children {
        let conns = mem.connections.get_by_subscription_id(&child.id());
        if let Some(conns) = conns {
            for (conn_id, _) in conns {
                let (up, down) = metrics.get_connection_total_traffic(&conn_id);
                total_uplink += up;
                total_downlink += down;
                connections_count += 1;
            }
        }
    }

    let active_children = children.iter().filter(|c| c.is_active()).count();
    let env = parent.scope_env().cloned();
    drop(mem);

    // Slot details and the term live in the registry, which the cache does not
    // carry; one small query per premium client is fine here.
    let (slot, premium_until) = match &env {
        Some(env) => match memory
            .db
            .premium_slots()
            .find_by_env(&env.to_string())
            .await
        {
            Ok(slot) => {
                let until = memory
                    .db
                    .premium_slots()
                    .premium_until_of(parent.id())
                    .await
                    .ok()
                    .flatten();
                (
                    slot.map(|slot| PremiumSlotInfo {
                        pool: slot.pool,
                        slot_no: slot.slot_no,
                        domain: slot.domain,
                        address: slot.address.map(|addr| addr.to_string()),
                        country: slot.country,
                    }),
                    until,
                )
            }
            Err(_) => (None, None),
        },
        None => (None, None),
    };

    Ok(warp::reply::json(&PremiumState {
        children_count: children.len(),
        active_children,
        connections_count,
        total_traffic: AdminSubscriptionTraffic {
            uplink: total_uplink,
            downlink: total_downlink,
        },
        env: env.map(|env| env.to_string()),
        premium_until,
        slot,
    }))
}

pub async fn premium_child_list_handler<N, C, S>(
    parent: S,
    memory: MemSync<N, C, S>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let mem = memory.memory.read().await;
    let children: Vec<PremiumChild> = mem
        .subscriptions
        .find_by_parent_id(&parent.id())
        .into_iter()
        .map(|s| PremiumChild {
            id: s.id(),
            refer_code: s.refer_code(),
            expires_at: s.expires_at(),
            is_active: s.is_active(),
            created_at: s.created_at(),
            limit_bytes: s.limit_bytes(),
        })
        .collect();

    Ok(warp::reply::json(&children))
}

pub async fn premium_create_child_handler<N, C, S>(
    parent: S,
    req: PremiumChildCreateRequest,
    trace_id_header: Option<String>,
    memory: MemSync<N, C, S>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let trace_id = subscription_audit::trace_id_from_header(trace_id_header);
    let sub_id = Uuid::new_v4();
    let ref_code = fcore::utils::get_uuid_last_octet_simple(&sub_id);

    if let Err(e) = req.validate() {
        return Ok(Box::new(warp::reply::with_status(
            warp::reply::json(
                &serde_json::json!({"status": 400, "message": e}),
            ),
            warp::http::StatusCode::BAD_REQUEST,
        )));
    }

    let expires_at = req.days.map(|d| Utc::now() + chrono::Duration::days(d));

    let mut sub = Subscription::new(sub_id, ref_code, expires_at, req.limit_bytes);
    sub.set_parent_id(parent.id());

    subscription_audit::log_transaction_start(sub_id, req.days);

    match SyncOp::add_sub(&memory, sub)
        .instrument(subscription_audit::transaction_span(
            "premium_create_child_handler",
            sub_id,
            Some(trace_id),
        ))
        .await
    {
        Ok(_) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "id": sub_id })),
            StatusCode::CREATED,
        ))),
        Err(e) => {
            tracing::error!("Failed to create premium child subscription: {:?}", e);
            Ok(Box::new(http::internal_error(&format!("{}", e))))
        }
    }
}

pub async fn premium_update_child_handler<N, C, S>(
    parent: S,
    child_id: Uuid,
    req: PremiumChildUpdateRequest,
    trace_id_header: Option<String>,
    memory: MemSync<N, C, S>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let trace_id = subscription_audit::trace_id_from_header(trace_id_header);
    {
        let mem = memory.memory.read().await;
        let child = mem.subscriptions.find_by_id(&child_id).ok_or_else(|| {
            warp::reject::custom(fcore::Error::Custom("Child not found".to_string()))
        })?;
        ensure_child_of(&parent, child)?;
    }

    let update_req = super::super::request::Subscription {
        days: req.days,
        refer_code: None,
        limit_bytes: req.limit_bytes,
    };

    subscription_audit::log_transaction_start(child_id, req.days);

    match SyncOp::update_sub(
        &memory,
        &child_id,
        update_req,
    )
    .instrument(subscription_audit::transaction_span(
        "premium_update_child_handler",
        child_id,
        Some(trace_id),
    ))
    .await
    {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "id": child_id })),
            StatusCode::OK,
        )),
        Err(e) => {
            tracing::error!("Failed to update premium child subscription: {:?}", e);
            Ok(http::internal_error(&format!("{}", e)))
        }
    }
}

pub async fn premium_child_connections_handler<N, C, S>(
    parent: S,
    child_id: Uuid,
    memory: MemSync<N, C, S>,
    metrics: Arc<MetricStorage>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let mem = memory.memory.read().await;
    let child = mem.subscriptions.find_by_id(&child_id).ok_or_else(|| {
        warp::reject::custom(fcore::Error::Custom("Child not found".to_string()))
    })?;
    ensure_child_of(&parent, child)?;

    let conns = mem.connections.get_by_subscription_id(&child_id);

    let result: Vec<PremiumConnection> = match conns {
        Some(cs) => cs
            .iter()
            .map(|(id, c)| {
                let (uplink, downlink) = metrics.get_connection_total_traffic(id);
                PremiumConnection {
                    id: *id,
                    env: c.get_env().to_string(),
                    proto: c.get_proto().proto().to_string(),
                    subscription_id: child_id,
                    is_deleted: c.get_deleted(),
                    traffic: AdminSubscriptionTraffic { uplink, downlink },
                }
            })
            .collect(),
        None => Vec::new(),
    };

    Ok(warp::reply::json(&result))
}

pub async fn premium_create_connection_handler<N, C, S>(
    parent: S,
    child_id: Uuid,
    req: PremiumConnectionCreateRequest,
    memory: MemSync<N, C, S>,
    wg_network: IpAddrMask,
    awg_network: IpAddrMask,
    awg_mobile_network: Option<IpAddrMask>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    ensure_scope(&parent, &req.env)?;

    let proto = {
        let mem = memory.memory.read().await;
        let child = mem.subscriptions.find_by_id(&child_id).ok_or_else(|| {
            warp::reject::custom(fcore::Error::Custom("Child not found".to_string()))
        })?;
        ensure_child_of(&parent, child)?;
        if !child.is_active() {
            return Err(warp::reject::custom(fcore::Error::Custom(
                "Child subscription is not active".to_string(),
            )));
        }

        match req.proto {
            // WG-family protocols share the allocation logic; each tag
            // draws from its own address pool.
            Tag::Wireguard | Tag::AmneziaWg | Tag::AmneziaWgMobile => {
                let network = match req.proto {
                    Tag::Wireguard => &wg_network,
                    Tag::AmneziaWg => &awg_network,
                    Tag::AmneziaWgMobile => match awg_mobile_network.as_ref() {
                        Some(n) => n,
                        None => {
                            return Ok(http::internal_error(
                                "amnezia_wireguard_mobile_network is not configured",
                            ))
                        }
                    },
                    _ => unreachable!(),
                };

                let last_ip: Option<Ipv4Addr> = mem
                    .connections
                    .get_last_addr(req.proto)
                    .and_then(|mask| mask.as_ipv4());

                let next = match last_ip {
                    Some(ip) => IpAddrMask::increment_ipv4(ip),
                    None => network.first_peer_ip(),
                };

                let next = match next {
                    Some(ip) => ip,
                    None => return Ok(http::internal_error("Failed to allocate IP")),
                };

                if !network.contains_ipv4(next) {
                    return Ok(http::internal_error("IP out of range"));
                }

                let param = WgParam {
                    keys: WgKeys::default(),
                    address: IpAddrMask {
                        address: IpAddr::V4(next),
                        cidr: 32,
                    },
                };

                match req.proto {
                    Tag::Wireguard => Proto::Wireguard { param },
                    Tag::AmneziaWg => Proto::AmneziaWg { param },
                    _ => Proto::AmneziaWgMobile { param },
                }
            }
            Tag::Shadowsocks => {
                let password = fcore::utils::generate_random_password(15);
                Proto::Shadowsocks { password }
            }
            Tag::VlessTcpReality
            | Tag::VlessGrpcReality
            | Tag::VlessXhttpReality
            | Tag::VlessXhttpCdn
            | Tag::Vmess => Proto::Xray(req.proto),
            Tag::Hysteria2 => {
                let token = Uuid::new_v4();
                Proto::Hysteria2 { token }
            }
            Tag::Mtproto => {
                let secret = fcore::utils::generate_random_password(15);
                Proto::Mtproto { secret }
            }
        }
    };

    let conn_id = Uuid::new_v4();
    let expires_at = req.days.map(|d| Utc::now() + chrono::Duration::days(d as i64));
    let conn = Connection::new(&req.env, Some(child_id), proto, expires_at);

    match SyncOp::add_conn(&memory, &conn_id, conn.clone(), None, None, None, true).await {
        Ok(Status::Ok(id)) | Ok(Status::AlreadyExist(id)) => {
            let msg = vec![conn.as_create_message(&conn_id)];
            let topic = if conn.get_token().is_some() {
                Some(Topic::Auth)
            } else if conn.get_proto().is_mtproto() {
                None
            } else {
                Some(conn.get_env().into())
            };

            if let Some(topic) = topic {
                let bytes = match rkyv::to_bytes::<_, 1024>(&msg) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!("Serialization error: {}", e);
                        return Ok(http::internal_error(&format!("Serialization error: {}", e)));
                    }
                };
                let _ = memory.publisher.send_binary(&topic, bytes.as_ref()).await;
            }

            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({ "id": id })),
                StatusCode::CREATED,
            ))
        }
        Ok(Status::BadRequest(id, msg)) => Ok(http::bad_request(&format!(
            "BadRequest {} {}",
            id, msg
        ))),
        Ok(status) => Ok(http::bad_request(&format!(
            "Unsupported operation status: {}",
            status
        ))),
        Err(err) => {
            tracing::error!("Internal error while processing connection {}: {}", conn_id, err);
            Ok(http::internal_error(&format!(
                "Internal error while processing connection {}: {}",
                conn_id, err
            )))
        }
    }
}

pub async fn premium_delete_connection_handler<N, C, S>(
    parent: S,
    conn_id: Uuid,
    memory: MemSync<N, C, S>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let conn: C = {
        let mem = memory.memory.read().await;
        let conn = mem.connections.get(&conn_id).ok_or_else(|| {
            warp::reject::custom(fcore::Error::Custom("Connection not found".to_string()))
        })?;
        let sub_id = conn
            .get_subscription_id()
            .ok_or_else(|| warp::reject::custom(fcore::Error::Custom("Invalid connection".to_string())))?;
        let child = mem.subscriptions.find_by_id(&sub_id).ok_or_else(|| {
            warp::reject::custom(fcore::Error::Custom("Subscription not found".to_string()))
        })?;
        ensure_child_of(&parent, child)?;
        conn.clone()
    };

    match SyncOp::delete_connection(&memory, &conn_id, &conn, Some(DELETE_REASON_MANUAL)).await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({})),
            StatusCode::NO_CONTENT,
        )),
        Err(e) => {
            tracing::error!("Failed to delete premium connection: {:?}", e);
            Ok(http::internal_error(&format!("{}", e)))
        }
    }
}

pub async fn premium_child_traffic_handler<N, C, S>(
    parent: S,
    child_id: Uuid,
    memory: MemSync<N, C, S>,
    metrics: Arc<MetricStorage>,
) -> Result<impl Reply, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq + From<Subscription>,
{
    let created_at = {
        let mem = memory.memory.read().await;
        let child = mem.subscriptions.find_by_id(&child_id).ok_or_else(|| {
            warp::reject::custom(fcore::Error::Custom("Child not found".to_string()))
        })?;
        ensure_child_of(&parent, child)?;
        child.created_at()
    };

    let traffic = build_subscription_traffic(&memory.db, &metrics, child_id, created_at).await?;
    Ok(warp::reply::json(&traffic))
}

/// Priority given to a request when the account has paid for premium.
///
/// The api cannot work this out by itself: payments live in the `payment`
/// database, which this service does not read. Until phase 3 wires the SKU,
/// priority is set by hand through `POST /admin/api/premium/requests/{id}/priority`;
/// the payment-gw will call the same endpoint when a premium payment is
/// confirmed.
#[allow(dead_code)]
const PRIORITY_PAYING: i16 = 10;

fn request_view(request: &fcore::PremiumRequest, position: Option<i64>) -> PremiumRequestView {
    PremiumRequestView {
        id: request.id,
        status: request.status.to_string(),
        months: request.months,
        position,
        offered_env: request.offered_env.clone(),
        offer_expires_at: request.offer_expires_at,
        created_at: request.created_at,
    }
}

/// `GET /premium/availability?subscription_id=…` — can this account get premium
/// right now, and if not, where is it in the queue?
///
/// Auth is the service token with an explicit subscription id, like `/share/sub`:
/// the whole point of the endpoint is to serve accounts that have **no** premium
/// token yet, so premium auth cannot apply. The ЛК reaches it through the
/// mrkting proxy.
pub async fn premium_availability_handler<N, C, S>(
    query: PremiumSubscriptionQuery,
    memory: MemSync<N, C, S>,
) -> Result<warp::reply::Response, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
{
    let subscription_id = query.subscription_id;

    {
        let mem = memory.memory.read().await;
        match mem.subscriptions.find_by_id(&subscription_id) {
            Some(sub) if !sub.is_deleted() => {}
            _ => return Err(warp::reject::not_found()),
        }
    }

    let stock = match memory.db.premium_slots().stock().await {
        Ok(stock) => stock,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "error": "stock_unavailable",
                    "message": format!("{e}"),
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response())
        }
    };

    let active = memory
        .db
        .premium_requests()
        .active_for(subscription_id)
        .await
        .unwrap_or(None);

    let (position, request) = match &active {
        Some(request) => {
            let position = memory
                .db
                .premium_requests()
                .position(request.id)
                .await
                .ok()
                .flatten();
            (position, Some(request_view(request, position)))
        }
        None => (None, None),
    };

    // An account may already hold premium: then the answer is "you have it",
    // not "join the queue". The env lives in the cache, the slot and the term in
    // the registry.
    let scope_env = {
        let mem = memory.memory.read().await;
        mem.subscriptions
            .find_by_id(&subscription_id)
            .and_then(|sub| sub.scope_env().cloned())
    };

    let (has_premium, premium_until, slot) = match &scope_env {
        Some(env) => {
            let env = env.to_string();
            let slot = memory
                .db
                .premium_slots()
                .find_by_env(&env)
                .await
                .ok()
                .flatten();
            let until = memory
                .db
                .premium_slots()
                .premium_until_of(subscription_id)
                .await
                .ok()
                .flatten();
            let info = slot.map(|slot| PremiumSlotInfo {
                pool: slot.pool,
                slot_no: slot.slot_no,
                domain: slot.domain,
                address: slot.address.map(|addr| addr.to_string()),
                country: slot.country,
            });
            (true, until, info)
        }
        None => (false, None, None),
    };

    let availability = PremiumAvailability {
        free: stock.free,
        total: stock.total,
        out_of_stock: stock.is_out_of_stock(),
        has_premium,
        premium_until,
        slot,
        waitlist_position: position,
        // A request only makes sense when there is nothing to hand out.
        can_request: !has_premium && stock.is_out_of_stock() && active.is_none(),
        request,
    };

    Ok(warp::reply::json(&availability).into_response())
}

/// `POST /premium/request` — leave the waitlist.
pub async fn premium_request_create_handler<N, C, S>(
    body: PremiumRequestCreateBody,
    memory: MemSync<N, C, S>,
) -> Result<warp::reply::Response, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
{
    let months = body.months.unwrap_or(1);
    if !(1..=120).contains(&months) {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "bad_months",
                "message": "months must be between 1 and 120",
            })),
            StatusCode::BAD_REQUEST,
        )
        .into_response());
    }

    {
        let mem = memory.memory.read().await;
        match mem.subscriptions.find_by_id(&body.subscription_id) {
            Some(sub) if !sub.is_deleted() => {}
            _ => return Err(warp::reject::not_found()),
        }
    }

    let stock = match memory.db.premium_slots().stock().await {
        Ok(stock) => stock,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "error": "stock_unavailable",
                    "message": format!("{e}"),
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response())
        }
    };

    // Everyone enters at priority 0. A confirmed premium payment (phase 3) or a
    // support decision bumps it via the admin endpoint — the api cannot see the
    // payment database, so it must not guess.
    let priority: i16 = 0;

    match memory
        .db
        .premium_requests()
        .create(body.subscription_id, months, body.contact.clone(), priority)
        .await
    {
        Ok(CreateOutcome::Created(request)) => {
            let position = memory
                .db
                .premium_requests()
                .position(request.id)
                .await
                .ok()
                .flatten();
            Ok(warp::reply::json(&serde_json::json!({
                "status": 200,
                "request": request_view(&request, position),
                "free": stock.free,
            }))
            .into_response())
        }
        // Idempotent: the same account asking twice keeps its place in line.
        Ok(CreateOutcome::AlreadyWaiting(request)) => {
            let position = memory
                .db
                .premium_requests()
                .position(request.id)
                .await
                .ok()
                .flatten();
            Ok(warp::reply::json(&serde_json::json!({
                "status": 200,
                "existing": true,
                "request": request_view(&request, position),
            }))
            .into_response())
        }
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "request_failed",
                "message": format!("{e}"),
            })),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response()),
    }
}

/// `GET /premium/request?subscription_id=…` — the account's own request.
pub async fn premium_request_get_handler<N, C, S>(
    query: PremiumSubscriptionQuery,
    memory: MemSync<N, C, S>,
) -> Result<warp::reply::Response, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
{
    match memory
        .db
        .premium_requests()
        .active_for(query.subscription_id)
        .await
    {
        Ok(Some(request)) => {
            let position = memory
                .db
                .premium_requests()
                .position(request.id)
                .await
                .ok()
                .flatten();
            Ok(warp::reply::json(&serde_json::json!({
                "request": request_view(&request, position),
            }))
            .into_response())
        }
        Ok(None) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "no_request",
                "request": serde_json::Value::Null,
            })),
            StatusCode::NOT_FOUND,
        )
        .into_response()),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "request_failed",
                "message": format!("{e}"),
            })),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response()),
    }
}

/// `DELETE /premium/request/{id}?subscription_id=…` — cancel your own request.
pub async fn premium_request_cancel_handler<N, C, S>(
    request_id: Uuid,
    query: PremiumSubscriptionQuery,
    memory: MemSync<N, C, S>,
) -> Result<warp::reply::Response, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
{
    match memory
        .db
        .premium_requests()
        .cancel(request_id, query.subscription_id)
        .await
    {
        Ok(true) => Ok(warp::reply::json(&serde_json::json!({
            "status": 200,
            "cancelled": true,
            "request_id": request_id,
        }))
        .into_response()),
        Ok(false) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "request_not_found",
                "request_id": request_id,
            })),
            StatusCode::NOT_FOUND,
        )
        .into_response()),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "request_failed",
                "message": format!("{e}"),
            })),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response()),
    }
}

/// `POST /premium/confirm?subscription_id=…` — accept the offered slot.
///
/// The client cannot call the admin convert endpoint, and it does not choose the
/// term: the request carries the months the person asked for.
pub async fn premium_request_confirm_handler<N, C, S>(
    query: PremiumSubscriptionQuery,
    memory: MemSync<N, C, S>,
) -> Result<warp::reply::Response, Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
{
    let Some(request) = memory
        .db
        .premium_requests()
        .active_for(query.subscription_id)
        .await
        .unwrap_or(None)
    else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "no_request"})),
            StatusCode::NOT_FOUND,
        )
        .into_response());
    };

    // Only an actual offer can be accepted: a waiting request has nothing to
    // confirm yet.
    if request.status != fcore::RequestStatus::Offered {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "no_offer",
                "message": "Nothing to confirm yet",
            })),
            StatusCode::CONFLICT,
        )
        .into_response());
    }

    let days = (request.months as i64).saturating_mul(30);
    let premium_until = chrono::Utc::now() + chrono::Duration::days(days);

    match memory
        .db
        .premium_requests()
        .convert(request.id, Some(premium_until))
        .await
    {
        Ok(ConvertOutcome::Converted {
            request,
            env,
            slot_key,
            token,
            premium_until,
        }) => {
            {
                let mut mem = memory.memory.write().await;
                if let Some(sub) = mem.subscriptions.find_by_id_mut(&request.subscription_id) {
                    if let Ok(parsed) = Env::parse_lenient(&env) {
                        sub.set_scope_env(parsed);
                    }
                    sub.set_premium_token(token);
                }
            }

            crate::subscription_audit::log_premium_converted(
                request.id,
                request.subscription_id,
                &env,
                &slot_key,
            );

            Ok(warp::reply::json(&serde_json::json!({
                "status": 200,
                "env": env,
                "slot_key": slot_key,
                "premium_until": premium_until,
            }))
            .into_response())
        }
        Ok(ConvertOutcome::NotActive) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "request_not_active"})),
            StatusCode::CONFLICT,
        )
        .into_response()),
        Ok(ConvertOutcome::OutOfStock) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "out_of_stock"})),
            StatusCode::CONFLICT,
        )
        .into_response()),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": format!("{e}")})),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response()),
    }
}
