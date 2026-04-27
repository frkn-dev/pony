use base64::Engine;
use chrono::DateTime;
use chrono::Utc;
use std::collections::HashSet;

use pony::http::helpers as http;
use pony::http::response::{EnvInfo, Instance, SubscriptionResponse};
use pony::http::ResponseMessage;
use warp::http::StatusCode;

use pony::{
    get_uuid_last_octet_simple, Connection, ConnectionApiOperations, ConnectionBaseOperations,
    ConnectionStorageApiOperations, Env, InboundConnLink, MetricStorage, NodeStorageOperations,
    Status, Subscription, SubscriptionOperations, SubscriptionStorageOperations, Tag,
};

use super::super::super::sync::tasks::SyncOp;
use super::super::super::sync::MemSync;
use super::super::param::SubIdQueryParam;
use super::super::param::SubQueryParam;
use super::super::request::Subscription as SubReq;

/// Handler creates subscription
// POST /subscription
pub async fn post_subscription_handler<N, C, S>(
    sub_req: SubReq,
    memory: MemSync<N, C, S>,
    bonus: i64,
    promo_codes: Vec<String>,
) -> Result<impl warp::Reply, warp::Rejection>
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
    let sub_id = uuid::Uuid::new_v4();
    let mut bonus_days = 0;

    let ref_code = sub_req
        .refer_code
        .unwrap_or_else(|| get_uuid_last_octet_simple(&sub_id));

    let sub_id_to_update = if let Some(ref_by) = sub_req.referred_by.clone() {
        let mem = memory.memory.read().await;
        let is_promo = promo_codes.iter().any(|c| c == &ref_by);

        if let Some(sub) = mem.subscriptions.find_by_refer_code(&ref_by) {
            if !is_promo {
                bonus_days = 7;
            }
            Some(sub.id())
        } else {
            return Ok(http::bad_request("Refer code no found"));
        }
    } else {
        None
    };

    if let Some(id) = sub_id_to_update {
        if let Err(e) = SyncOp::add_days(&memory, &id, bonus).await {
            return Ok(http::internal_error(&format!(
                "Couldn't create subscription: {}",
                e
            )));
        }
    }

    let expires_at: Option<DateTime<Utc>> = sub_req
        .days
        .map(|days| Utc::now() + chrono::Duration::days(days + bonus_days));

    let sub = Subscription::new(sub_id, sub_req.referred_by, ref_code, expires_at);

    match SyncOp::add_sub(&memory, sub.clone()).await {
        Ok(Status::Ok(id)) => Ok(http::success_response(
            format!("Subscription {} has been created", id),
            Some(sub_id),
            Instance::Subscription(sub),
        )),
        Ok(Status::AlreadyExist(id)) => Ok(http::not_modified(&format!(
            "Subscription {} already exists",
            id
        ))),
        Ok(Status::NotFound(id)) => Ok(http::not_found(&format!(
            "Subscription {} is not found",
            id
        ))),
        Err(err) => Ok(http::internal_error(&format!(
            "Internal error while processing subscription {}: {}",
            sub_id, err
        ))),
        _ => Ok(http::not_modified("")),
    }
}

// Handler updates subscription
// PUT /subscription
pub async fn put_subscription_handler<N, C, S>(
    sub_param: SubIdQueryParam,
    sub_req: SubReq,
    memory: MemSync<N, C, S>,
) -> Result<impl warp::Reply, warp::Rejection>
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
    let sub_id = sub_param.id;

    match SyncOp::update_sub(&memory, &sub_id, sub_req).await {
        Ok(Status::Updated(id)) => Ok(http::success_response(
            format!("Subscription {} has been updated", id),
            Some(sub_id),
            Instance::None,
        )),
        Ok(Status::NotFound(id)) => Ok(http::not_found(&format!(
            "Subscription {} is not found",
            id
        ))),
        Err(err) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: 500,
                message: format!(
                    "Internal error while processing subscription {}: {}",
                    sub_id, err
                ),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
        _ => Ok(http::not_modified("")),
    }
}

///get subscription_info_json
pub async fn get_subscription_info_json<N, C, S>(
    subscription_id: uuid::Uuid,
    memory: MemSync<N, C, S>,
    _metrics: std::sync::Arc<MetricStorage>,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + std::fmt::Debug
        + PartialEq,
{
    let mem = memory.memory.read().await;

    let Some(sub) = mem.subscriptions.find_by_id(&subscription_id) else {
        return Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&"Subscription not found"),
            warp::http::StatusCode::NOT_FOUND,
        )));
    };

    let connections = mem.connections.get_by_subscription_id(&subscription_id);
    let mut locations = Vec::new();

    if let Some(conns) = connections.clone() {
        let active_envs: HashSet<Env> = conns
            .iter()
            .filter(|(_, conn)| !conn.get_deleted())
            .map(|(_, conn)| conn.get_env())
            .collect();

        for env in active_envs {
            let mut has_xray = false;
            let mut has_hysteria = false;
            let mut has_mtproto = false;

            let nodes = mem.nodes.get_by_env(&env);

            let xray_tags = [
                Tag::VlessGrpcReality,
                Tag::VlessTcpReality,
                Tag::VlessXhttpReality,
            ];

            let xray_nodes = nodes.clone();
            let xray_node_exists = xray_nodes.is_some_and(|ns| {
                ns.iter()
                    .any(|n| n.inbounds.values().any(|i| xray_tags.contains(&i.tag)))
            });

            let mtproto_nodes = nodes.clone();
            let mtproto_node_exist = mtproto_nodes.is_some_and(|ns| {
                ns.iter()
                    .any(|n| n.inbounds.values().any(|i| i.tag == Tag::Mtproto))
            });

            let hyst_node_exists = nodes.is_some_and(|ns| {
                ns.iter()
                    .any(|n| n.inbounds.values().any(|i| i.tag == Tag::Hysteria2))
            });

            for (_, conn) in conns.clone() {
                if !conn.get_deleted() && conn.get_env() == env {
                    let proto = conn.get_proto().proto();
                    if xray_node_exists && xray_tags.contains(&proto) {
                        has_xray = true;
                    }
                    if hyst_node_exists && proto == Tag::Hysteria2 {
                        has_hysteria = true;
                    }

                    if mtproto_node_exist && proto == Tag::Mtproto {
                        has_mtproto = true;
                    }
                }
            }

            locations.push(EnvInfo {
                env,
                has_xray,
                has_hysteria,
                has_mtproto,
            });
        }
    }

    let sub_resp = SubscriptionResponse {
        id: sub.id(),
        expires: sub.expires_at().unwrap_or_default(),
        days: sub.days_remaining().unwrap_or(0),
        ref_code: sub.refer_code(),
        invited_count: mem.subscriptions.count_invited_by(&sub.refer_code()),
        locations,
    };

    Ok(Box::new(warp::reply::json(&sub_resp)))
}

pub async fn subscription_link_handler_new<N, C, S>(
    sub_param: SubQueryParam,
    memory: MemSync<N, C, S>,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + std::fmt::Debug
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static + PartialEq,
    Connection: From<C>,
{
    let mem = memory.memory.read().await;

    if let Some(sub) = mem.subscriptions.find_by_id(&sub_param.id) {
        if !sub.is_active() {
            return Ok(Box::new(http::not_found(&format!(
                "Subscription {} is expired",
                sub_param.id
            ))));
        }
    }

    let conns = mem.connections.get_by_subscription_id(&sub_param.id);
    let mut links = vec![];

    let tags = sub_param.proto.tags();

    if let Some(conns) = conns {
        for (conn_id, conn) in conns {
            if conn.get_deleted() || conn.get_env() != sub_param.env {
                continue;
            }
            if !tags.contains(&conn.get_proto().proto()) {
                continue;
            }

            if let Some(nodes) = mem.nodes.get_by_env(&conn.get_env()) {
                for node in nodes.iter() {
                    if let Some(inbound) = node.inbounds.get(&conn.get_proto().proto().clone()) {
                        if let Ok(link) = inbound.create_link(
                            &conn_id,
                            &conn.clone().into(),
                            &node.address,
                            &node.label,
                        ) {
                            links.push(link);
                        }
                    }
                }
            }
        }
    }

    if links.is_empty() {
        return Ok(Box::new(http::not_found(&format!(
            "Nodes for subscription {} not found",
            sub_param.id
        ))));
    }

    match sub_param.format.as_str() {
        "txt" => {
            let body = links.join("\n");
            Ok(Box::new(warp::reply::with_status(
                warp::reply::with_header(body, "Content-Type", "text/plain"),
                StatusCode::OK,
            )))
        }
        "base64" => {
            let sub = base64::engine::general_purpose::STANDARD.encode(links.join("\n"));
            Ok(Box::new(warp::reply::with_status(
                warp::reply::with_header(format!("{}\n", sub), "Content-Type", "text/plain"),
                StatusCode::OK,
            )))
        }
        _ => Ok(Box::new(warp::reply::with_status(
            warp::reply::with_header("BAD REQUEST", "Content-Type", "text/plain"),
            StatusCode::BAD_REQUEST,
        ))),
    }
}
