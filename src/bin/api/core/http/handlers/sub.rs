use base64::Engine;
use chrono::DateTime;
use chrono::Utc;

use warp::http::Response;
use warp::http::StatusCode;

use pony::http::helpers as http;
use pony::http::requests::SubCreateReq;
use pony::http::requests::SubIdQueryParam;
use pony::http::requests::SubQueryParam;
use pony::http::requests::SubUpdateReq;
use pony::http::ResponseMessage;
use pony::utils;
use pony::xray_op::clash::generate_clash_config;
use pony::xray_op::clash::generate_proxy_config;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStorageApiOp;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::Subscription;
use pony::SubscriptionOp;
use pony::SubscriptionStorageOp;
use pony::Tag;

use crate::core::sync::tasks::SyncOp;
use crate::core::sync::MemSync;

/// Handler creates subscription
// POST /subscription
pub async fn post_subscription_handler<N, C, S>(
    sub_req: SubCreateReq,
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
    let sub_id = uuid::Uuid::new_v4();
    let expires_at: Option<DateTime<Utc>> = sub_req
        .days
        .map(|days| Utc::now() + chrono::Duration::days(days.into()));

    let sub = Subscription::new(sub_id, sub_req.referred_by, expires_at);

    match SyncOp::add_sub(&memory, sub.clone()).await {
        Ok(StorageOperationStatus::Ok(id)) => Ok(http::success_response(
            format!("Subscription {} has been created", id),
            Some(sub_id),
            http::Instance::Subscription(sub),
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
    sub_req: SubUpdateReq,
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

    match SyncOp::update_sub(&memory, &sub_id, sub_req.clone()).await {
        Ok(StorageOperationStatus::Updated(id)) => Ok(http::success_response(
            format!("Subscription {} has been updated", id),
            Some(sub_id),
            http::Instance::None,
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

// GET /sub/stat?id=<>
pub async fn subscription_conn_stat_handler<N, C, S>(
    sub_param: SubIdQueryParam,
    memory: MemSync<N, C, S>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq,
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
{
    log::debug!("Received: {:?}", sub_param);

    let mem = memory.memory.read().await;
    let mut result: Vec<(uuid::Uuid, ConnectionStat, Tag)> = Vec::new();

    if mem.subscriptions.find_by_id(&sub_param.id).is_none() {
        return Ok(http::not_found("Subscription is not found"));
    }

    if let Some(connections) = mem.connections.get_by_subscription_id(&sub_param.id) {
        for (conn_id, conn) in connections {
            let tag = conn.get_proto().proto();

            let stat = ConnectionStat {
                online: conn.get_online(),
                downlink: conn.get_downlink(),
                uplink: conn.get_uplink(),
            };

            result.push((conn_id, stat, tag));
        }

        Ok(http::success_response(
            "List of subscription connection statistics".to_string(),
            None,
            http::Instance::Stat(result),
        ))
    } else {
        Ok(http::not_found("Connections is not found"))
    }
}

/// Gets Subscription info page
// GET /sub/info?id=&env=
pub async fn subscription_info_handler<N, C, S>(
    sub_param: SubQueryParam,
    memory: MemSync<N, C, S>,
    host: String,
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

    fn format_bytes(bytes: i64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;
        const TB: f64 = GB * 1024.0;

        let b = bytes as f64;
        if b >= TB {
            format!("{:.2} ТБ", b / TB)
        } else if b >= GB {
            format!("{:.2} ГБ", b / GB)
        } else if b >= MB {
            format!("{:.2} МБ", b / MB)
        } else if b >= KB {
            format!("{:.2} КБ", b / KB)
        } else {
            format!("{} байт", bytes)
        }
    }

    let env = &sub_param.env;
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

    let invited = mem.subscriptions.count_invited_by(&sub.referral_code());

    let mut downlink = 0;
    let mut uplink = 0;

    if let Some(conns) = mem.connections.get_by_subscription_id(id) {
        for (_, c) in conns {
            downlink += c.get_downlink();
            uplink += c.get_uplink();
        }
    }

    let down_str = format_bytes(downlink);
    let up_str = format_bytes(uplink);

    let base_link = format!("{}/sub?id={}&env={}", host, id, env);
    let main_link = format!("{}/sub?id={}&format=txt&env={}", host, id, env);

    let html = format!(
r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Информация о подписке</title>
<style>
body {{
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    background: #0f172a;
    color: #e5e7eb;
    max-width: 720px;
    margin: 40px auto;
    padding: 24px;
}}
.card {{
    background: #020617;
    border-radius: 12px;
    padding: 24px;
    box-shadow: 0 0 0 1px #1e293b;
}}
h1, h2, h3 {{
    margin-top: 0;
}}
.stat {{
    margin: 8px 0;
}}
.status-active {{
    color: #22c55e;
    font-weight: 600;
}}
.status-expired {{
    color: #ef4444;
    font-weight: 600;
}}
a {{
    color: #38bdf8;
    text-decoration: none;
}}
button {{
    margin-top: 6px;
    padding: 6px 12px;
    border-radius: 6px;
    border: none;
    background: #1e293b;
    color: #e5e7eb;
    cursor: pointer;
}}
button:hover {{
    background: #334155;
}}
.qr {{
    margin-top: 16px;
    text-align: center;
}}
.small {{
    font-size: 13px;
    color: #94a3b8;
}}
ul {{
    padding-left: 20px;
}}
</style>
</head>
<body>

<div class="card">
<h1>Подписка</h1>

<div class="stat">Статус: <span class="{status_class}">{status_text}</span></div>
<div class="stat">Дата окончания: {expires}</div>
<div class="stat">Осталось дней: {days}</div>

<hr>

<div class="stat">Трафик: ↓ {down_str} &nbsp;&nbsp; ↑ {up_str}</div>

<hr>

<h3>Ссылки для подключения</h3>

<a href="{base_link}&format=plain" target="_blank">Универсальная ссылка (рекомендуется)</a>
<br>
<button onclick="copyText('{base_link}&format=plain')">Скопировать ссылку</button>

<p><b>Дополнительные форматы:</b></p>
<ul>
<li><a href="{base_link}&format=txt" target="_blank">TXT</a>  для v2ray</li>
<li><a href="{base_link}&format=clash" target="_blank">Clash</a> — для Clash / Clash Meta</li>
</ul>

<hr>

<h3>Поддерживаемые приложения</h3>
<ul>
<li> iOS / macOS — Shadowrocket, Streisand, Clash Verge</li>
<li> Android — Hiddify, v2rayNG, Nekobox</li>
<li> Windows — Hiddify, v2rayN, Clash Verge</li>
<li> Linux — Clash, v2ray-core</li>
</ul>

<hr>

<div class="qr">
<h3>QR-код</h3>
<canvas id="qr"></canvas>

<div class="small">Отсканируйте в VPN-приложении</div>
</div>

<hr>

<h3>Реферальная программа</h3>
<div class="stat">Ваш реферальный код: <b>{ref}</b>
 <button onclick="copyText('{ref}')">Скопировать код</button>
</div>

<div class="small">Вы пригласили: {invited}</div>

</div>

<br><hr>
<div class=small>  <a href=https://t.me/frkn_support>Поддержка</a></div>

<script src="https://cdn.jsdelivr.net/npm/qrcode/build/qrcode.min.js"></script>

<script>
function copyText(text) {{
    navigator.clipboard.writeText(text).then(() => {{
        alert("Скопировано");
    }});
}}
window.onload = () => {{
    QRCode.toCanvas(
        document.getElementById("qr"),
        "{main_link}",
        {{ width: 220 }}
    );
}};
</script>

</body>
</html>"#,
        status_class = status_class,
        status_text = status_text,
        expires = expires,
        days = days,
        down_str = down_str,
        up_str = up_str,
        base_link = base_link,
        ref = sub.referral_code(),
        invited = invited
    );

    Ok(Box::new(warp::reply::with_status(
        warp::reply::with_header(html, "Content-Type", "text/html; charset=utf-8"),
        StatusCode::OK,
    )))
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
        None => {
            return Ok(http::not_found(&format!(
                "Connections {}  are not found",
                sub_param.id
            )));
        }
        Some(c) => {
            let cons: Vec<_> = c
                .iter()
                .filter(|(_id, conn)| conn.get_deleted() == false)
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
                http::Instance::Connections(cons.clone()),
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
        } else {
            return Ok(Box::new(http::not_found(&format!(
                "Subscription {}  are not found",
                sub_param.id
            ))));
        }
    }

    let conns = mem.connections.get_by_subscription_id(&sub_param.id);
    let mut inbounds_by_node = vec![];

    let env = sub_param.env;

    if let Some(conns) = conns {
        for (conn_id, conn) in conns {
            if conn.get_deleted() {
                continue;
            }
            if conn.get_env() != env {
                continue;
            }

            if let Some(nodes) = mem.nodes.get_by_env(&conn.get_env()) {
                for node in nodes.iter() {
                    if let Some(inbound) = &node.inbounds.get(&conn.get_proto().proto()) {
                        inbounds_by_node.push((
                            inbound.as_inbound_response(),
                            conn_id,
                            node.label.clone(),
                            node.address,
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

            for (inbound, conn_id, label, address) in &inbounds_by_node {
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

            return Ok(Box::new(response));
        }

        "txt" => {
            let links = inbounds_by_node
                .iter()
                .filter_map(|(inbound, conn_id, label, ip)| {
                    utils::create_conn_link(inbound.tag, conn_id, inbound.clone(), label, *ip).ok()
                })
                .collect::<Vec<_>>();

            let body = links.join("\n");

            return Ok(Box::new(warp::reply::with_status(
                warp::reply::with_header(body, "Content-Type", "text/plain"),
                StatusCode::OK,
            )));
        }

        _ => {
            let links = inbounds_by_node
                .iter()
                .filter_map(|(inbound, conn_id, label, ip)| {
                    utils::create_conn_link(inbound.tag, conn_id, inbound.clone(), label, *ip).ok()
                })
                .collect::<Vec<_>>();

            let sub = base64::engine::general_purpose::STANDARD.encode(links.join("\n"));
            let body = format!("{}\n", sub);

            return Ok(Box::new(warp::reply::with_status(
                warp::reply::with_header(body, "Content-Type", "text/plain"),
                StatusCode::OK,
            )));
        }
    }
}
