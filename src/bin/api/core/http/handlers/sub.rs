use warp::http::StatusCode;

use pony::http::requests::SubIdQueryParam;
use pony::http::requests::SubQueryParam;
use pony::http::ResponseMessage;
use pony::utils;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStorageApiOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;
use pony::Tag;

use crate::core::sync::MemSync;

// GET /sub/stat?id=<>
pub async fn subscription_conn_stat_handler<N, C, S>(
    sub_param: SubIdQueryParam,
    memory: MemSync<N, C, S>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    N: NodeStorageOp + Sync + Send + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
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

        let response = ResponseMessage {
            status: StatusCode::OK.as_u16(),
            message: "List of subscription connection statistics".to_string(),
            response: result,
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<Option<Vec<(uuid::Uuid, ConnectionStat, Tag)>>> {
            status: StatusCode::NOT_FOUND.as_u16(),
            message: "Connections not found".to_string(),
            response: Some(vec![]),
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}

/// Gets Subscriprion link
// GET /sub/info?id=
pub async fn subscription_info_handler<C>(
    sub_param: SubQueryParam,
    host: String,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection>
where
    C: ConnectionApiOp
        + ConnectionBaseOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + std::fmt::Debug
        + PartialEq,
{
    let env = &sub_param.env;
    let id = &sub_param.id;
    let ref_code = utils::get_uuid_last_octet_simple(&id);

    let types = vec!["txt", "clash", "plain"];

    let mut links_html = String::new();
    links_html.push_str("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Subscription Links</title></head><body>");
    links_html.push_str("<h1>Subscription Links</h1><ul>");

    for t in types {
        let link = format!("{}/sub?id={}&format={}&env={}", host, id, t, env);
        links_html.push_str(&format!(
            "<li>{} <a href=\"{}\" target=\"_blank\">{}</a></li><br>",
            t, link, link
        ));
    }

    let ref_str = format!(
        "</ul> <br><br> Your Referal code: {} </body></html>",
        ref_code
    );
    links_html.push_str(&ref_str);

    Ok(Box::new(warp::reply::with_status(
        warp::reply::with_header(links_html, "Content-Type", "text/html; charset=utf-8"),
        StatusCode::OK,
    )))
}
