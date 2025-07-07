use base64::Engine;
use defguard_wireguard_rs::net::IpAddrMask;
use warp::http::Response;
use warp::http::StatusCode;

use pony::http::requests::ConnCreateRequest;
use pony::http::requests::ConnQueryParam;

use pony::http::requests::ConnUpdateRequest;
use pony::http::requests::UserIdQueryParam;
use pony::http::requests::UserSubQueryParam;
use pony::http::IdResponse;
use pony::http::ResponseMessage;
use pony::state::node::Status as NodeStatus;
use pony::state::storage::connection::ApiOp;
use pony::utils;
use pony::xray_op::clash::generate_clash_config;
use pony::xray_op::clash::generate_proxy_config;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Conn as Connection;
use pony::ConnWithId;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::ConnectionStatus;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::Proto;
use pony::Tag;
use pony::WgParam;

use crate::core::sync::tasks::SyncOp;
use crate::core::sync::SyncState;

/// Handler creates connection
// POST /connection
pub async fn create_connection_handler<N, C>(
    conn_req: ConnCreateRequest,
    publisher: ZmqPublisher,
    state: SyncState<N, C>,
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
{
    let env = conn_req.env.clone();
    let conn_id = uuid::Uuid::new_v4();
    let mem = state.memory.lock().await;

    if conn_req.password.is_some() && conn_req.wg.is_some() {
        let response = ResponseMessage::<Option<String>> {
            status: StatusCode::BAD_REQUEST.as_u16(),
            message: "Cannot specify both password (Shadowsocks) and wg (WireGuard)".into(),
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    if !conn_req.proto.is_wireguard() && conn_req.wg.is_some() {
        let response = ResponseMessage::<Option<String>> {
            status: StatusCode::BAD_REQUEST.as_u16(),
            message: "Wg params are allowed only for Wireguard proto".into(),
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    if !conn_req.proto.is_wireguard() && conn_req.node_id.is_some() {
        let response = ResponseMessage::<Option<String>> {
            status: StatusCode::BAD_REQUEST.as_u16(),
            message: "node_id param are allowed only for Wireguard proto".into(),
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    let node_id = if conn_req.proto.is_wireguard() {
        match conn_req.node_id {
            Some(node_id) => {
                let node_valid = mem.nodes.get_by_id(&node_id).is_some_and(|n| {
                    n.env == conn_req.env && n.inbounds.contains_key(&conn_req.proto)
                });

                if !node_valid {
                    let response = ResponseMessage::<Option<String>> {
                        status: StatusCode::BAD_REQUEST.as_u16(),
                        message:
                            "node_id doesn't exist, mismatched env or missing WireGuard inbound"
                                .into(),
                        response: None,
                    };
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&response),
                        StatusCode::BAD_REQUEST,
                    ));
                }

                Some(node_id)
            }
            None => {
                mem.nodes
                    .select_least_loaded_node(&conn_req.env, &conn_req.proto, &mem.connections)
            }
        }
    } else {
        None
    };

    if conn_req.password.is_some() && !conn_req.proto.is_shadowsocks() {
        let response = ResponseMessage::<Option<String>> {
            status: StatusCode::BAD_REQUEST.as_u16(),
            message: format!(
                "Password is only allowed for Shadowsocks, but got {:?}",
                conn_req.proto
            ),
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    if let (Some(wg), Some(node_id)) = (&conn_req.wg, node_id) {
        let address_taken = mem.connections.values().any(|c| {
            if let Proto::Wireguard {
                param,
                node_id: existing_node_id,
            } = c.get_proto()
            {
                existing_node_id == node_id && param.address.ip == wg.address.ip
            } else {
                false
            }
        });

        if wg.address.cidr > 32 {
            let response = ResponseMessage::<Option<String>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: "Invalid CIDR: must be 0..=32".into(),
                response: None,
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ));
        }

        if address_taken {
            let response = ResponseMessage::<Option<String>> {
                status: StatusCode::CONFLICT.as_u16(),
                message: "Address already taken for this node_id".into(),
                response: None,
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::CONFLICT,
            ));
        }

        if let Some(node) = mem.nodes.get_by_id(&node_id) {
            if let Some(inbound) = node.inbounds.get(&conn_req.proto) {
                if let Some(wg_settings) = &inbound.wg {
                    if !utils::ip_in_mask(&wg_settings.network, wg.address.ip) {
                        let response = ResponseMessage::<Option<String>> {
                            status: StatusCode::BAD_REQUEST.as_u16(),
                            message: format!("Address out of node netmask {}", wg_settings.network),
                            response: None,
                        };
                        return Ok(warp::reply::with_status(
                            warp::reply::json(&response),
                            StatusCode::BAD_REQUEST,
                        ));
                    }
                }
            }
        }
    }

    let wg_param = if conn_req.proto.is_wireguard() && conn_req.wg.is_none() {
        let node_id = match node_id {
            Some(id) => id,
            None => {
                let response = ResponseMessage::<Option<String>> {
                    status: StatusCode::BAD_REQUEST.as_u16(),
                    message: "Missing node_id for WireGuard".into(),
                    response: None,
                };
                return Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::BAD_REQUEST,
                ));
            }
        };

        let node = match mem.nodes.get_by_id(&node_id) {
            Some(n) => n,
            None => {
                let response = ResponseMessage::<Option<String>> {
                    status: StatusCode::BAD_REQUEST.as_u16(),
                    message: "Node not found".into(),
                    response: None,
                };
                return Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::BAD_REQUEST,
                ));
            }
        };

        let inbound = match node.inbounds.get(&conn_req.proto) {
            Some(i) => i,
            None => {
                let response = ResponseMessage::<Option<String>> {
                    status: StatusCode::BAD_REQUEST.as_u16(),
                    message: "Inbound for proto not found".into(),
                    response: None,
                };
                return Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::BAD_REQUEST,
                ));
            }
        };

        let wg_settings = match &inbound.wg {
            Some(wg) => wg,
            None => {
                let response = ResponseMessage::<Option<String>> {
                    status: StatusCode::BAD_REQUEST.as_u16(),
                    message: "WireGuard settings missing".into(),
                    response: None,
                };
                return Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::BAD_REQUEST,
                ));
            }
        };

        let base_ip = node
            .inbounds
            .get(&Tag::Wireguard)
            .and_then(|inb| inb.wg.as_ref())
            .map(|wg| wg.address)
            .and_then(utils::increment_ip);

        let max_ip = mem
            .connections
            .iter()
            .filter(|(_, conn)| conn.get_proto().proto() == Tag::Wireguard)
            .filter_map(|(_, conn)| conn.get_wireguard().map(|wg| wg.address.ip))
            .filter_map(utils::to_ipv4)
            .max();

        let next_ip = match max_ip
            .and_then(utils::increment_ip)
            .or_else(|| base_ip.and_then(utils::increment_ip))
            .map(std::net::IpAddr::V4)
        {
            Some(ip) => {
                log::debug!("IP Gen: {:?} {:?} {:?}", base_ip, max_ip, ip);
                ip
            }
            None => {
                let response = ResponseMessage::<Option<String>> {
                    status: StatusCode::BAD_REQUEST.as_u16(),
                    message: "Failed to generate next IP".into(),
                    response: None,
                };
                return Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::BAD_REQUEST,
                ));
            }
        };

        if !utils::ip_in_mask(&wg_settings.network, next_ip) {
            let response = ResponseMessage::<Option<String>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: format!(
                    "Generated address {} is out of node netmask {}",
                    next_ip, wg_settings.network
                ),
                response: None,
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ));
        }

        Some(WgParam::new(IpAddrMask::new(next_ip, 32)))
    } else {
        None
    };

    drop(mem);

    log::debug!("WG params {:?}", wg_param);
    let proto = if let Some(wg) = &conn_req.wg {
        Proto::Wireguard {
            param: wg.clone(),
            node_id: node_id.unwrap(),
        }
    } else if let Some(wg) = wg_param {
        Proto::Wireguard {
            param: wg.clone(),
            node_id: node_id.unwrap(),
        }
    } else if let Some(password) = &conn_req.password {
        Proto::Shadowsocks {
            password: password.clone(),
        }
    } else {
        Proto::Xray(conn_req.proto)
    };

    let conn: Connection = Connection::new(
        conn_req.trial.unwrap_or(true),
        conn_req.limit.unwrap_or(0),
        &env,
        ConnectionStatus::Active,
        conn_req.user_id,
        ConnectionStat::default(),
        proto,
        node_id,
    )
    .into();

    log::debug!("New connection to create {}", conn);

    let msg = conn.as_create_message(&conn_id);

    match SyncOp::add_conn(&state, &conn_id, conn.clone()).await {
        Ok(StorageOperationStatus::Ok(id)) => {
            if let Some(node_id) = conn.node_id {
                let _ = publisher.send(&node_id.to_string(), msg).await;
            } else {
                let _ = publisher.send(&env, msg).await;
            }
            let response = ResponseMessage {
                status: StatusCode::OK.as_u16(),
                message: format!("Connection {} has been created", id),
                response: Some(ConnWithId { id: id, conn: conn }),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }

        Ok(StorageOperationStatus::AlreadyExist(id)) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::NOT_MODIFIED.as_u16(),
                message: format!("Connection {} already exists", id),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_MODIFIED,
            ))
        }

        Ok(StorageOperationStatus::BadRequest(id, msg)) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: format!("BadRequest {} {}", id, msg),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ))
        }

        Ok(status) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: format!("Unsupported operation status: {}", status),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ))
        }

        Err(err) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                message: format!(
                    "Internal error while processing connection {}: {}",
                    conn_id, err
                ),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Handler deletes connection
// DELETE /connection?conn_id=
pub async fn delete_connection_handler<N, C>(
    conn_param: ConnQueryParam,
    publisher: ZmqPublisher,
    state: SyncState<N, C>,
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
{
    let conn_id = conn_param.conn_id;
    let conn_opt = {
        let mem = state.memory.lock().await;
        mem.connections.get(&conn_id).cloned()
    };

    let Some(conn) = conn_opt else {
        let response = ResponseMessage::<Option<uuid::Uuid>> {
            status: StatusCode::NOT_FOUND.as_u16(),
            message: format!("Connection {} not found", conn_id),
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ));
    };

    if conn.get_deleted() {
        let response = ResponseMessage::<Option<uuid::Uuid>> {
            status: StatusCode::NOT_FOUND.as_u16(),
            message: format!("Connection {} is deleted", conn_id),
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ));
    }

    match SyncOp::delete_connection(&state, &conn_id).await {
        Ok(StorageOperationStatus::Ok(id)) => {
            let msg = conn.as_delete_message(&conn_id);

            if let Some(node_id) = conn.get_wireguard_node_id() {
                let _ = publisher.send(&node_id.to_string(), msg).await;
            } else {
                let _ = publisher.send(&conn.get_env(), msg).await;
            }

            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::OK.as_u16(),
                message: format!("Connection {} has been deleted", id),
                response: Some(id),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }

        Ok(StorageOperationStatus::NotFound(id)) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::NOT_FOUND.as_u16(),
                message: format!("Connection {} not found", id),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ))
        }

        Ok(status) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::BAD_REQUEST.as_u16(),
                message: format!("Unsupported operation status: {}", status),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ))
        }

        Err(err) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                message: format!(
                    "Internal error while deleting connection {}: {}",
                    conn_id, err
                ),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Handler updates connection
// PUT /connection
pub async fn put_connection_handler<N, C>(
    conn_id: ConnQueryParam,
    conn_req: ConnUpdateRequest,
    publisher: ZmqPublisher,
    state: SyncState<N, C>,
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
{
    let conn_id = conn_id.conn_id;
    log::debug!("Connection to update {}", conn_id);

    match SyncOp::update_conn(&state, &conn_id, conn_req).await {
        Ok(StorageOperationStatus::Updated(id)) => {
            let mem = state.memory.lock().await;

            let conn = mem.connections.get(&id);
            if let Some(conn) = conn {
                let msg = conn.as_update_message(&id);

                let _ = publisher.send(&conn.get_env(), msg).await;

                let message = format!("Connection {} has been updated", id);
                let response = ResponseMessage::<Option<IdResponse>> {
                    status: 200,
                    message,
                    response: Some(IdResponse { id }),
                };
                Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::OK,
                ))
            } else {
                let message = format!("Connection {} is not found", id);
                let response = ResponseMessage::<Option<uuid::Uuid>> {
                    status: 404,
                    message,
                    response: None,
                };
                Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::NOT_FOUND,
                ))
            }
        }
        Ok(StorageOperationStatus::NotModified(id)) => {
            let message = format!("Connection {} is identical", id);
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: 304,
                message,
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_MODIFIED,
            ))
        }
        Ok(StorageOperationStatus::NotFound(id)) => {
            let message = format!("Connection {} is not found", id);
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: 404,
                message,
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ))
        }
        Ok(StorageOperationStatus::BadRequest(id, msg)) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: 400,
                message: format!("BadRequest {} {}", id, msg),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ))
        }
        Ok(status) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: 400,
                message: format!("Unsupported operation status: {}", status),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            ))
        }
        Err(err) => {
            let response = ResponseMessage::<Option<uuid::Uuid>> {
                status: 500,
                message: format!(
                    "Internal error while processing connection {}: {}",
                    conn_id, err
                ),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Get list of user connection credentials
pub async fn get_user_connections_handler<N, C>(
    user_req: UserIdQueryParam,
    state: SyncState<N, C>,
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
{
    log::debug!("Received: {:?}", user_req);

    let user_id = user_req.user_id;

    let connections = {
        let mem = state.memory.lock().await;
        mem.connections.get_by_user_id(&user_id)
    };

    match connections {
        None => {
            let message = format!("Connections are not found");
            let response = ResponseMessage::<Option<Vec<&(uuid::Uuid, C)>>> {
                status: StatusCode::NOT_FOUND.as_u16(),
                message,
                response: Some(vec![]),
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ));
        }
        Some(c) => {
            let cons: Vec<_> = c
                .iter()
                .filter(|(_id, conn)| conn.get_deleted() == false)
                .collect();

            let message = format!("Connections are found for {}", user_id);
            let response = ResponseMessage::<Option<Vec<&(uuid::Uuid, C)>>> {
                status: StatusCode::OK.as_u16(),
                message,
                response: Some(cons),
            };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ));
        }
    }
}

/// Get connection detaisl
// GET /connection?conn_id=
pub async fn get_connection_handler<N, C>(
    conn_req: ConnQueryParam,
    state: SyncState<N, C>,
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
        + PartialEq
        + serde::ser::Serialize,
{
    let state = state.memory.lock().await;

    let conn_id = conn_req.conn_id;

    if let Some(conn) = state.connections.get(&conn_id) {
        let message = format!("Connections are found");
        let response = ResponseMessage::<Option<(uuid::Uuid, C)>> {
            status: StatusCode::OK.as_u16(),
            message,
            response: Some((conn_id, conn.clone())),
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ));
    } else {
        let message = format!("Connections are not found");
        let response = ResponseMessage::<Option<(uuid::Uuid, C)>> {
            status: StatusCode::NOT_FOUND.as_u16(),
            message,
            response: None,
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ));
    }
}

pub async fn subscription_link_handler<N, C>(
    user_req: UserSubQueryParam,
    state: SyncState<N, C>,
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
{
    let mem = state.memory.lock().await;

    let conns = mem.connections.get_by_user_id(&user_req.user_id);
    let mut inbounds_by_node = vec![];

    if let Some(conns) = conns {
        for (conn_id, conn) in conns {
            if conn.get_deleted() || conn.get_status() != ConnectionStatus::Active {
                continue;
            }
            if let Some(nodes) = mem.nodes.get_by_env(&conn.get_env()) {
                for node in nodes.iter().filter(|n| n.status == NodeStatus::Online) {
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
        let message = format!("Connections are not found");
        let response = ResponseMessage::<Option<Vec<&(uuid::Uuid, Connection)>>> {
            status: StatusCode::NOT_FOUND.as_u16(),
            message,
            response: Some(vec![]),
        };
        return Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        )));
    }

    match user_req.format.as_str() {
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
