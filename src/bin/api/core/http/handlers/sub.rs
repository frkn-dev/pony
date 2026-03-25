use base64::Engine;
use chrono::DateTime;
use chrono::Utc;
use pony::mtproto_op::mtproto_conn;
use url::Url;

use super::super::param::MtprotoQueryParam;
use super::super::request::TagReq;
use warp::http::Response;
use warp::http::StatusCode;

use super::super::param::SubIdQueryParam;
use super::super::param::SubQueryParam;
use super::super::request::Subscription as SubReq;
use pony::http::helpers as http;
use pony::http::ResponseMessage;
use pony::utils;
use pony::utils::get_uuid_last_octet_simple;
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

    let is_promo = promo_codes.iter().any(|c| c == &ref_code);

    let sub_id_to_update = if let Some(ref_by) = sub_req.referred_by.clone() {
        let mem = memory.memory.read().await;

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
    web_host: String,
    api_web_host: String,
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

    let invited = mem.subscriptions.count_invited_by(&sub.refer_code());

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

    let is_ru = env == "ru";

    let title = if is_ru {
        "Подписка на Рилзопровод (RU)"
    } else {
        "Подписка на Рилзопровод"
    };

    let ru_link = format!("{}/sub/info?id={}&env={}", api_web_host, id, "ru");
    let main_link = format!("{}/sub/info?id={}", api_web_host, id);
    let ru_block = if is_ru {
        format!(
            r#"<a href="{main_link}" class="small text-link right">Мне нужны иностранные сервера</a>"#
        )
    } else {
        format!(
            r#"<a href="{ru_link}" class="small text-link right">Мне нужны российские сервера</a>"#
        )
    };

    let base_link = format!("{}/sub?id={}&env={}", api_web_host, id, env);
    let base_link_mtproto = format!("{}/sub/mtproto?id={}&env={}", api_web_host, id, env);
    let main_link_vless = format!(
        "{}/sub?id={}&format=txt&env={}&proto=Xray",
        api_web_host, id, env
    );
    let main_link_h2 = format!(
        "{}/sub?id={}&format=txt&env={}&proto=Hysteria2",
        api_web_host, id, env
    );

    let html = format!(
r#"{head}
<div class="card">
{ru_block}
<header>
  <div class="logo">
    <img src="{logo}" alt="FRKN Logo" />
    <a href="{web_host}">FRKN</a>
  </div>
</header>
<body>
<h1>{title}</h1>
<div class="stat">Статус: <span class="{status_class}">{status_text}</span></div>
<div class="stat">Дата окончания: <span id="expires">{expires}</span></div>
<div class="stat">
  Осталось дней: <span id="days">{days}</span>
  <a class="link-btn small" id="scrollToAdd" style="margin-left:8px;">Докинуть</a>
</div>
<div class="small-id">Id: <b>{subscription_id}</b></div>
<hr>
<div class="stat small-id">Трафик: ↓ {down_str} &nbsp;&nbsp; ↑ {up_str}</div>
<hr>

<h3>Ссылки для подключения</h3>

<div class="tabs">
    <button class="tab active" data-tab="xray">Xray</button>
    <button class="tab" data-tab="hysteria">Hysteria2</button>
    <button class="tab" data-tab="mtproto">MTproto</button>
    <button class="tab" data-tab="wg">Wireguard</button>
    <button class="tab" data-tab="awg">Amnezia Wireguard</button>
    <button class="tab" data-tab="tt">TrustTunnel</button>
</div>

<div id="tab-xray" class="tab-content active">
    <ul class="proxy-list">
        <li class="proxy-item" onclick="copyText('{base_link}&format=plain&proto=Xray')">
            <div class="proxy-label">Универсальная</div>
            <div class="proxy-action">Скопировать</div>
        </li>
        <li class="proxy-item" onclick="copyText('{base_link}&format=txt&proto=Xray')">
            <div class="proxy-label">TXT</div>
            <div class="proxy-action">Скопировать</div>
        </li>
        <li class="proxy-item" onclick="copyText('{base_link}&format=clash&proto=Xray')">
            <div class="proxy-label">Clash</div>
            <div class="proxy-action">Скопировать</div>
        </li>
    </ul>

    <div class="qr">
        <canvas id="qr"></canvas>
        <div class="small">Отсканируйте в приложении</div>
    </div>
    <br><br>

    <div class="small">
       <h3>Поддерживаемые приложения</h3>
       <ul>
       <li>Happ, Hiddify, v2rayNG, Shadowrocket, Streisand, Clash Verge, Nekobox</li>
       </ul>
    </div>
</div>

<div id="tab-hysteria" class="tab-content">
    <ul class="proxy-list">
         <li class="proxy-item" onclick="copyText('{base_link}&format=plain&proto=Hysteria2')">
             <div class="proxy-label">Универсальная</div>
             <div class="proxy-action">Скопировать</div>
         </li>
        <li class="proxy-item" onclick="copyText('{base_link}&format=txt&proto=Hysteria2')">
            <div class="proxy-label">TXT</div>
            <div class="proxy-action">Скопировать</div>
        </li>
    </ul>

    <div class="qr">
        <canvas id="qr2"></canvas>
        <div class="small">Отсканируйте в приложении</div>
    </div>

    <br><br>
    <div class="small">
        <h3>Поддерживаемые приложения</h3>
        <ul>
            <li>Shadowrocket, hiddify, v2rayN</li>
        </ul>
    </div>
</div>

<div id="tab-mtproto" class="tab-content">
    <a href="{base_link_mtproto}" target="_blank">Bonus Track: Telegram Proxy - Открыть</a>

    <br><br>
    <div class="small">
        <h3>Поддерживаемые приложения</h3>
        <p> Телеграм поддерживает ссылки mtproto напрямую</p>
    </div>
</div>

<div id="tab-wg" class="tab-content">
    Wireguard скоро будет доступен
    <br><br>
    <div class="small">
        <h3>Поддерживаемые приложения</h3>
        <p> </p>
    </div>
</div>

<div id="tab-awg" class="tab-content">
    Amnezia Wireguard скоро будет доступен
    <br><br>
    <div class="small">
        <h3>Поддерживаемые приложения</h3>
        <p> </p>
    </div>
</div>

<div id="tab-tt" class="tab-content">
    TrustTunnel скоро будет доступен
    <br><br>
    <div class="small">
        <h3>Поддерживаемые приложения</h3>
        <p> </p>
    </div>
</div>

<hr>
<div class="key" id="key">
<h3>Докинуть дней (Активировать ключ) </h3>

<div class="stat">
    <input id="keyInput" placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-X" style="padding:8px; width:270px;" />
    <button onclick="activateKey()">Активировать</button><br><br>
    &nbsp;&nbsp; <a href="{web_host}/activation-keys.html" target="_blank" class="small text-link">
           Что такое ключ активации и где его взять?
       </a>
</div>

<div id="keyResult" class="small"></div>
</div>

<hr>
<h3>Реферальная программа</h3>
<div class="stat">Твой реферальный код: <b>{ref}</b><br>
 <button onclick="copyText('{ref}')">Скопировать код</button>
 <button onclick="copyText('{web_host}/?code={ref}#subscribe')">Скопировать ссылку для друга</button></div>


<div class="small">Вы пригласили: {invited} </div>

<div class="small">Добавим по 7 дней доступа и тебе и другу</a></div>


<br><hr>

{footer}

</div>

<script src="https://cdn.jsdelivr.net/npm/qrcode/build/qrcode.min.js"></script>

<script>
document.querySelectorAll(".tab").forEach(btn => {{
    btn.onclick = () => {{
        const name = btn.dataset.tab;

        document.querySelectorAll(".tab").forEach(el => el.classList.remove("active"));
        document.querySelectorAll(".tab-content").forEach(el => el.classList.remove("active"));

        btn.classList.add("active");
        document.getElementById("tab-" + name).classList.add("active");
    }};
}});
</script>

<script>
const scrollBtn = document.getElementById("scrollToAdd");
const keySection = document.getElementById("key");

if (scrollBtn && keySection) {{
    scrollBtn.addEventListener("click", () => {{
        keySection.scrollIntoView({{ behavior: "smooth" }});
    }});
}}
</script>

<script>async function activateKey() {{
    const key = document.getElementById("keyInput").value;
    const resultEl = document.getElementById("keyResult");

    if (!key) {{
        resultEl.innerText = "Введите ключ";
        return;
    }}

    try {{
        const res = await fetch("/key/activate", {{
            method: "POST",
            headers: {{
                "Content-Type": "application/json"
            }},
            body: JSON.stringify({{
                code: key,
                subscription_id: "{subscription_id}"
            }})
        }});

        const data = await res.json();

        if (res.ok && data.status === 200 && data.response) {{
            const {{ days, expires_at }} = data.response;

            if (days && expires_at) {{
                document.getElementById("days").innerText = days;
                document.getElementById("expires").innerText = expires_at;
            }}
            setTimeout(() => location.reload(), 1000);
            const key = data.response?.instance?.Key;
            if (key) {{
                const addedDays = key.days;
                resultEl.innerText = `✅ Ключ активирован! +${{addedDays}} дней`;
            }}
        }} else {{
            resultEl.innerText = "❌ " + data.message;
        }}
    }} catch (e) {{
        resultEl.innerText = "Ошибка сети";
    }}
}}

function showToast(text) {{
const toast = document.getElementById("toast");
   if (!toast) return;

   toast.innerText = text;
   toast.classList.add("show");

   setTimeout(() => {{
       toast.classList.remove("show");
   }}, 2000);
}}

function copyText(text) {{
    navigator.clipboard.writeText(text).then(() => {{
        showToast("Скопировано");
    }});
}}

window.onload = () => {{
    QRCode.toCanvas(
        document.getElementById("qr"),
        "{main_link_vless}",
        {{ width: 220 }}
    );

    QRCode.toCanvas(
        document.getElementById("qr2"),
        "{main_link_h2}",
        {{ width: 220 }}
    );
}};
</script>
<div id="toast" class="toast">Скопировано</div>
</body>
</html>"#,
    head = HEAD,
    footer = FOOTER,
            status_class = status_class,
            status_text = status_text,
            expires = expires,
            days = days,
            down_str = down_str,
            up_str = up_str,
            base_link = base_link,
            ref = sub.refer_code(),
            invited = invited,
            subscription_id = id,
            title = title,
            ru_block = ru_block,
            logo = LOGO,

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

/// Gets Subscriprion link
// GET /sub/mtproto?id=
pub async fn mtproto_link_handler<N, C, S>(
    param: MtprotoQueryParam,
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

    let nodes = mem.nodes.get_by_env(&param.env);

    if mem.subscriptions.get(&param.id).is_none() {
        return Ok(Box::new(http::not_found(&format!(
            "Subscription {} is not found",
            param.id
        ))));
    };

    let links: Vec<String> = nodes
        .iter()
        .flat_map(|node_vec| node_vec.iter())
        .filter_map(|node| {
            node.inbounds
                .values()
                .find(|inb| inb.tag == Tag::Mtproto)
                .and_then(|inbound| mtproto_conn(node.address, inbound, &node.label).ok())
        })
        .collect();

    let html_links = links
        .iter()
        .filter_map(|l| {
            let url = Url::parse(l).ok()?;

            let label = url
                .fragment()
                .map(|f| {
                    percent_encoding::percent_decode_str(f)
                        .decode_utf8_lossy()
                        .to_string()
                })
                .unwrap_or_else(|| "Telegram Proxy".into());

            Some(format!(
                "<li class=\"mt-proxy-item\"><a href=\"{href}\">
                <span class=\"proxy-label\">{label}</span>
                <span class=\"proxy-action\">Connect</span>
                </a></li>",
                href = l,
                label = label
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        r#"{head}
<body>

<div class="card">
<h1>Bonus Tack: Mtproto (tg-proxy)</h1>

<hr>

<h3>Ссылки для подключения</h3>
<ul class="proxy-list">{html_links}</ul>
<br><br><br>
<hr>
{footer}
</div>

</body>
</html>"#,
        head = HEAD,
        footer = FOOTER,
        html_links = html_links
    );

    Ok(Box::new(warp::reply::with_status(
        warp::reply::with_header(html, "Content-Type", "text/html; charset=utf-8"),
        StatusCode::OK,
    )))
}
