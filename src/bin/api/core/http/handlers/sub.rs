use std::collections::HashSet;

use url::Url;

use base64::Engine;
use chrono::DateTime;
use chrono::Utc;

use pony::metrics::storage::MetricStorage;
use warp::http::Response;
use warp::http::StatusCode;

use pony::http::helpers as http;
use pony::http::response::Instance;
use pony::http::ResponseMessage;
use pony::mtproto_op::mtproto_conn;
use pony::utils;
use pony::utils::get_uuid_last_octet_simple;
use pony::xray_op::clash::generate_clash_config;
use pony::xray_op::clash::generate_proxy_config;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStorageApiOp;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::Subscription;
use pony::SubscriptionOp;
use pony::SubscriptionStorageOp;
use pony::Tag;

use super::super::param::SubIdQueryParam;
use super::super::param::SubQueryParam;
use super::super::request::Subscription as SubReq;
use super::super::request::TagReq;
use super::html::{FOOTER, HEAD, LOGO};

use crate::core::sync::tasks::SyncOp;
use crate::core::sync::MemSync;

/// Handler creates subscription
// POST /subscription
pub async fn post_subscription_handler<N, C, S>(
    sub_req: SubReq,
    memory: MemSync<N, C, S>,
    bonus: i64,
    promo_codes: Vec<String>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOp
        + Send
        + Sync
        + Clone
        + 'static
        + std::cmp::PartialEq
        + std::convert::From<pony::Subscription>,
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
        Ok(StorageOperationStatus::Ok(id)) => Ok(http::success_response(
            format!("Subscription {} has been created", id),
            Some(sub_id),
            Instance::Subscription(sub),
        )),
        Ok(StorageOperationStatus::AlreadyExist(id)) => Ok(http::not_modified(&format!(
            "Subscription {} already exists",
            id
        ))),
        Ok(StorageOperationStatus::NotFound(id)) => Ok(http::not_found(&format!(
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
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOp
        + Send
        + Sync
        + Clone
        + 'static
        + std::cmp::PartialEq
        + std::convert::From<pony::Subscription>,
{
    let sub_id = sub_param.id;

    match SyncOp::update_sub(&memory, &sub_id, sub_req).await {
        Ok(StorageOperationStatus::Updated(id)) => Ok(http::success_response(
            format!("Subscription {} has been updated", id),
            Some(sub_id),
            Instance::None,
        )),
        Ok(StorageOperationStatus::NotFound(id)) => Ok(http::not_found(&format!(
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

/// Gets Subscription info page
// GET /sub/info?id=&env=
pub async fn subscription_info_handler<N, C, S>(
    sub_param: SubQueryParam,
    memory: MemSync<N, C, S>,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + std::fmt::Debug
        + PartialEq,
{
    use chrono::Utc;
    use warp::http::StatusCode;

    fn format_datetime(dt: chrono::DateTime<Utc>) -> String {
        dt.format("%d %B %Y · %H:%M UTC").to_string()
    }

    let env = &sub_param.env;
    let is_ru = env == "ru";
    let is_wl = env == "wl";
    let id = &sub_param.id;

    let mem = memory.memory.read().await;

    let Some(sub) = mem.subscriptions.find_by_id(id) else {
        return Ok(Box::new(http::not_found("Подписка не найдена")));
    };

    let is_active = sub.is_active();
    let status_text = if is_active {
        "Активна"
    } else {
        "Истекла"
    };
    let status_class = if is_active {
        "status-active"
    } else {
        "status-expired"
    };

    let expires = sub
        .expires_at()
        .map(format_datetime)
        .unwrap_or_else(|| "Без ограничения".into());

    let days = sub
        .days_remaining()
        .map(|d| d.max(0).to_string())
        .unwrap_or_else(|| "∞".into());

    let invited = mem.subscriptions.count_invited_by(&sub.refer_code());

    let title = if is_ru {
        "Подписка на Рилзопровод (RU)"
    } else if is_wl {
        "Подписка на Рилзопровод (БС)"
    } else {
        "Подписка на Рилзопровод"
    };

    let sub_link = format!("https://api.frkn.org/sub/info?id={}", id);
    let ru_link = format!("{}&env={}", sub_link, "ru");
    let wl_link = format!("{}&env={}", sub_link, "wl");
    let main_link = format!("{}&env={}", sub_link, "dev");

    let html = format!(
        r#"{head}
<div class="card">

<header>
  <div class="logo">
    <img src="{logo}" alt="FRKN Logo" />
    <a href="https://frkn.org">FRKN</a>
  </div>
</header>
<body>
<h1>{title}</h1>
</br>
<h3>Твоя Новая Страница подписки теперь доступна по АДРЕСУ </h3>

<h3>https://frkn.org/subscription?id={subscription_id}</h3>


<a href="https://frkn.org/subscription?id={subscription_id}"> https://frkn.org/subscription?id={subscription_id}</a></h1>
<br>
<div class="stat">Статус: <span class="{status_class}">{status_text}</span></div>
<div class="stat">Дата окончания: <span id="expires">{expires}</span></div>
<div class="stat">
  Осталось дней: <span id="days">{days}</span>
  <a class="link-btn small" id="scrollToAdd" style="margin-left:8px;">Докинуть</a>
</div>
<div class="small-id">Id: <b>{subscription_id}</b></div>
<hr>

{footer}
</div>


</body>
</html>"#,
        head = HEAD,
        footer = FOOTER,
        logo = LOGO,
        status_class = status_class,
        status_text = status_text,
        expires = expires,
        days = days,
        subscription_id = id,
        title = title,
    );

    Ok(Box::new(warp::reply::with_status(
        warp::reply::with_header(html, "Content-Type", "text/html; charset=utf-8"),
        StatusCode::OK,
    )))
}

#[derive(serde::Serialize)]
pub struct EnvInfo {
    pub env: String,
    pub has_xray: bool,
    pub has_hysteria: bool,
}

#[derive(serde::Serialize)]
pub struct SubscriptionResponse {
    pub id: uuid::Uuid,
    pub expires: DateTime<Utc>,
    pub days: i64,
    pub ref_code: String,
    pub invited_count: usize,
    pub locations: Vec<EnvInfo>,
}

/// get subscription_info_json
pub async fn get_subscription_info_json<N, C, S>(
    subscription_id: uuid::Uuid,
    memory: MemSync<N, C, S>,
    _metrics: std::sync::Arc<MetricStorage>,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq,
    C: ConnectionApiOp
        + ConnectionBaseOp
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
        let active_envs: HashSet<String> = conns
            .iter()
            .filter(|(_, conn)| !conn.get_deleted())
            .map(|(_, conn)| conn.get_env())
            .collect();

        for env in active_envs {
            let xray_tags = [
                Tag::VlessGrpcReality,
                Tag::VlessTcpReality,
                Tag::VlessXhttpReality,
            ];

            let nodes = mem.nodes.get_by_env(&env);
            let xray_nodes = nodes.clone();
            let xray_node_exists = xray_nodes.is_some_and(|ns| {
                ns.iter()
                    .any(|n| n.inbounds.values().any(|i| xray_tags.contains(&i.tag)))
            });

            let hyst_node_exists = nodes.is_some_and(|ns| {
                ns.iter()
                    .any(|n| n.inbounds.values().any(|i| i.tag == Tag::Hysteria2))
            });

            let mut has_xray = false;
            let mut has_hysteria = false;

            for (_, conn) in conns.clone() {
                if !conn.get_deleted() && conn.get_env() == env {
                    let proto = conn.get_proto().proto();
                    if xray_node_exists && xray_tags.contains(&proto) {
                        has_xray = true;
                    }
                    if hyst_node_exists && proto == Tag::Hysteria2 {
                        has_hysteria = true;
                    }
                }
            }

            locations.push(EnvInfo {
                env,
                has_xray,
                has_hysteria,
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

/// Get list of subscription connection credentials
pub async fn get_subscription_connections_handler<N, C, S>(
    sub_param: SubIdQueryParam,
    memory: MemSync<N, C, S>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + std::fmt::Debug
        + serde::ser::Serialize
        + PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq,
    pony::Connection: From<C>,
{
    log::debug!("Received: {:?}", sub_param);

    let sub_id = sub_param.id;

    let env = if let Some(env) = sub_param.env {
        env
    } else {
        "dev".to_string()
    };

    {
        let mem = memory.memory.read().await;
        if mem.subscriptions.find_by_id(&sub_param.id).is_none() {
            return Ok(http::not_found(&format!(
                "Subscription {}  is not found",
                sub_param.id
            )));
        }
    }

    let connections = {
        let mem = memory.memory.read().await;
        mem.connections.get_by_subscription_id(&sub_id)
    };

    match connections {
        None => Ok(http::not_found(&format!(
            "Connections {}  are not found",
            sub_param.id
        ))),
        Some(c) => {
            let cons: Vec<_> = c
                .iter()
                .filter(|(_id, conn)| !conn.get_deleted())
                .filter(|(_id, conn)| conn.get_env() == env)
                .map(|(id, conn)| (*id, conn.clone().into()))
                .collect();

            if cons.is_empty() {
                return Ok(http::not_found(&format!(
                    "Connections {}  are not found",
                    sub_param.id
                )));
            }
            Ok(http::success_response(
                format!("Connections are found for {}", sub_id),
                None,
                Instance::Connections(cons.clone()),
            ))
        }
    }
}

/// Gets Subscriprion link
// GET /sub?id=
pub async fn subscription_link_handler<N, C, S>(
    sub_param: SubQueryParam,
    memory: MemSync<N, C, S>,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + std::fmt::Debug
        + PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq,
{
    let mem = memory.memory.read().await;

    if let Some(sub) = mem.subscriptions.find_by_id(&sub_param.id) {
        if !sub.is_active() {
            return Ok(Box::new(http::not_found(&format!(
                "Subscription {}  are expired",
                sub_param.id
            ))));
        }
    }

    let conns = mem.connections.get_by_subscription_id(&sub_param.id);
    let mut inbounds_by_node = vec![];

    let env = sub_param.env;

    let tags = match sub_param.proto {
        TagReq::Xray => vec![
            Tag::VlessTcpReality,
            Tag::VlessGrpcReality,
            Tag::VlessXhttpReality,
            Tag::Vmess,
            Tag::Shadowsocks,
        ],
        TagReq::Wireguard => vec![Tag::Wireguard],
        TagReq::Hysteria2 => vec![Tag::Hysteria2],
        TagReq::Mtproto => vec![Tag::Mtproto],
    };

    if let Some(conns) = conns {
        for (conn_id, conn) in conns {
            if conn.get_deleted() {
                continue;
            }
            if conn.get_env() != env {
                continue;
            }

            let proto = conn.get_proto().proto();

            if !tags.contains(&proto) {
                continue;
            }

            let token = conn.get_token();

            if let Some(nodes) = mem.nodes.get_by_env(&conn.get_env()) {
                for node in nodes.iter() {
                    if let Some(inbound) = node.inbounds.get(&conn.get_proto().proto()) {
                        inbounds_by_node.push((
                            inbound.clone(),
                            conn_id,
                            node.label.clone(),
                            node.address,
                            token,
                        ));
                    }
                }
            }
        }
    }

    if inbounds_by_node.is_empty() {
        return Ok(Box::new(http::not_found(&format!(
            "Nodes for subscription {}  are not found",
            sub_param.id
        ))));
    }

    match sub_param.format.as_str() {
        "clash" => {
            let mut proxies = vec![];

            for (inbound, conn_id, label, address, _) in &inbounds_by_node {
                if let Some(proxy) = generate_proxy_config(inbound, *conn_id, *address, label) {
                    proxies.push(proxy)
                }
            }

            let config = generate_clash_config(proxies);
            let yaml = serde_yaml::to_string(&config)
                .unwrap_or_else(|_| "---\nerror: failed to serialize\n".into());

            let response = Response::builder()
                .header("Content-Type", "application/yaml")
                .status(StatusCode::OK)
                .body(yaml);

            Ok(Box::new(response))
        }

        "txt" => {
            let links = inbounds_by_node
                .iter()
                .filter_map(|(inbound, conn_id, label, ip, token)| {
                    utils::create_conn_link(inbound.tag, conn_id, inbound, label, *ip, token).ok()
                })
                .collect::<Vec<_>>();

            let body = links.join("\n");

            Ok(Box::new(warp::reply::with_status(
                warp::reply::with_header(body, "Content-Type", "text/plain"),
                StatusCode::OK,
            )))
        }

        _ => {
            let links = inbounds_by_node
                .iter()
                .filter_map(|(inbound, conn_id, label, ip, token)| {
                    utils::create_conn_link(inbound.tag, conn_id, inbound, label, *ip, token).ok()
                })
                .collect::<Vec<_>>();

            let sub = base64::engine::general_purpose::STANDARD.encode(links.join("\n"));
            let body = format!("{}\n", sub);

            Ok(Box::new(warp::reply::with_status(
                warp::reply::with_header(body, "Content-Type", "text/plain"),
                StatusCode::OK,
            )))
        }
    }
}
