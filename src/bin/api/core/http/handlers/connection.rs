use chrono::DateTime;
use chrono::Utc;
use defguard_wireguard_rs::net::IpAddrMask;
use rkyv::to_bytes;
use warp::http::StatusCode;

use pony::http::helpers as http;
use pony::http::requests::ConnCreateRequest;
use pony::http::requests::ConnQueryParam;
use pony::http::requests::ConnTypeParam;
use pony::http::requests::ConnUpdateRequest;
use pony::http::IdResponse;
use pony::http::IpParseError;
use pony::http::MyRejection;
use pony::http::ResponseMessage;
use pony::memory::tag::ProtoTag;
use pony::utils;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStat;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::Proto;
use pony::SubscriptionOp;
use pony::SubscriptionStorageOp;
use pony::Tag;
use pony::WgParam;

use crate::core::sync::tasks::SyncOp;
use crate::core::sync::MemSync;

/// Handler get connection
// GET /connections
pub async fn get_connections_handler<N, C, S>(
    conn_req: ConnTypeParam,
    publisher: ZmqPublisher,
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
    let mem = memory.memory.read().await;
    let proto = conn_req.proto;
    let env = conn_req.env;

    let last_update = conn_req.last_update;

    let connections_to_send: Vec<_> = mem
        .connections
        .iter()
        .filter(|(_, conn)| {
            if conn.get_deleted() {
                return false;
            }

            if conn.get_proto().proto() != proto {
                return false;
            }

            if let Some(ts) = last_update {
                conn.get_modified_at().and_utc().timestamp() as u64 >= ts
            } else {
                true
            }
        })
        .collect();

    log::debug!(
        "Sending {} {:?} connections to auth",
        connections_to_send.len(),
        proto
    );

    let messages: Vec<_> = connections_to_send
        .iter()
        .map(|(conn_id, conn)| conn.as_create_message(conn_id))
        .collect();

    let bytes = to_bytes::<_, 1024>(&messages).map_err(|e| {
        log::error!("Serialization error: {}", e);
        warp::reject::custom(MyRejection(Box::new(e)))
    })?;

    publisher
        .send_binary(&env, bytes.as_ref())
        .await
        .map_err(|e| {
            log::error!("Publish error: {}", e);
            warp::reject::custom(MyRejection(Box::new(e)))
        })?;

    let resp = ResponseMessage::<Option<IdResponse>> {
        status: StatusCode::OK.as_u16(),
        message: "Ok".to_string(),
        response: None,
    };
    Ok(warp::reply::with_status(
        warp::reply::json(&resp),
        StatusCode::OK,
    ))
}

/// Handler creates connection
// POST /connection
pub async fn create_connection_handler<N, C, S>(
    conn_req: ConnCreateRequest,
    publisher: ZmqPublisher,
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
    if let Err(e) = conn_req.validate() {
        return Ok(http::bad_request(&e));
    }

    let expired_at: Option<DateTime<Utc>> = conn_req
        .days
        .map(|days| Utc::now() + chrono::Duration::days(days.into()));

    let mem = memory.memory.read().await;
    if let Some(sub_id) = conn_req.subscription_id {
        if mem.subscriptions.find_by_id(&sub_id).is_none() {
            return Ok(http::bad_request(&format!(
                "Subscription {} not found",
                sub_id
            )));
        }
    }

    let proto = match conn_req.proto {
        ProtoTag::Wireguard => {
            let node_id = {
                match conn_req.node_id {
                    Some(node_id) => {
                        let node_valid = mem.nodes.get_by_id(&node_id).is_some_and(|n| {
                            n.env == conn_req.env && n.inbounds.contains_key(&conn_req.proto)
                        });

                        if !node_valid {
                            return Ok(http::bad_request(
                        "node_id doesn't exist, mismatched env or missing WireGuard inbound",
                    ));
                        }

                        node_id
                    }
                    None => {
                        if let Some(node_id) = mem.nodes.select_least_loaded_node(
                            &conn_req.env,
                            &conn_req.proto,
                            &mem.connections,
                        ) {
                            node_id
                        } else {
                            return Ok(http::not_found("Node not found for  WireGuard connection"));
                        }
                    }
                }
            };

            let wg_param = if let Some(wg_param) = conn_req.wg {
                if wg_param.address.cidr > 32 {
                    return Ok(http::bad_request("Invalid CIDR: must be 0..=32"));
                }
                let address_taken = mem.connections.values().any(|c| {
                    if let Proto::Wireguard {
                        param,
                        node_id: existing_node_id,
                    } = c.get_proto()
                    {
                        existing_node_id == node_id && param.address.addr == wg_param.address.addr
                    } else {
                        false
                    }
                });
                if address_taken {
                    return Ok(http::conflict("Address already taken for this node_id"));
                }
                if let Some(node) = mem.nodes.get_by_id(&node_id) {
                    if let Some(inbound) = node.inbounds.get(&conn_req.proto) {
                        if let Some(wg_settings) = &inbound.wg {
                            let ip = wg_param
                                .address
                                .addr
                                .parse()
                                .map_err(|e| warp::reject::custom(IpParseError(e)))?;
                            if !utils::ip_in_mask(&wg_settings.network, ip) {
                                return Ok(http::bad_request(&format!(
                                    "Address out of node netmask {}",
                                    wg_settings.network
                                )));
                            }
                        }
                    }
                }
                wg_param
            } else {
                let node = match mem.nodes.get_by_id(&node_id) {
                    Some(n) => n,
                    None => {
                        return Ok(http::not_found("Node not found"));
                    }
                };

                let inbound = match node.inbounds.get(&conn_req.proto) {
                    Some(i) => i,
                    None => {
                        return Ok(http::not_found("Inbound for proto not found"));
                    }
                };

                let wg_settings = match &inbound.wg {
                    Some(wg) => wg,
                    None => {
                        return Ok(http::bad_request("WireGuard settings missing"));
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
                    .filter_map(|(_, conn)| {
                        conn.get_wireguard()
                            .and_then(|wg| wg.address.addr.parse().ok())
                    })
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
                        return Ok(http::bad_request("Failed to generate next IP"));
                    }
                };

                if !utils::ip_in_mask(&wg_settings.network, next_ip) {
                    return Ok(http::bad_request(&format!(
                        "Generated address {} is out of node netmask {}",
                        next_ip, wg_settings.network
                    )));
                }

                WgParam::new(IpAddrMask::new(next_ip, 32))
            };

            Proto::Wireguard {
                param: wg_param,
                node_id: node_id,
            }
        }
        ProtoTag::Shadowsocks => Proto::Shadowsocks {
            password: conn_req.password.unwrap(),
        },
        ProtoTag::VlessTcpReality
        | ProtoTag::VlessGrpcReality
        | ProtoTag::VlessXhttpReality
        | ProtoTag::Vmess => Proto::Xray(conn_req.proto),
        ProtoTag::Hysteria2 => Proto::Hysteria2 {
            token: conn_req.token.unwrap(),
        },
        ProtoTag::Mtproto => unreachable!("Mtproto handled earlier"),
    };

    drop(mem);

    let conn: Connection = Connection::new(
        &conn_req.env,
        conn_req.subscription_id,
        ConnectionStat::default(),
        proto,
        expired_at,
    )
    .into();

    log::debug!("New connection to create {}", conn);
    let conn_id = uuid::Uuid::new_v4();
    let msg = conn.as_create_message(&conn_id);

    let mut messages = vec![];
    messages.push(msg);

    match SyncOp::add_conn(&memory, &conn_id, conn.clone()).await {
        Ok(StorageOperationStatus::Ok(id)) => {
            let bytes = match rkyv::to_bytes::<_, 1024>(&messages) {
                Ok(b) => b,
                Err(e) => {
                    return Ok(http::internal_error(&format!("Serialization error: {}", e)));
                }
            };

            let topic = if let Some(node_id) = conn.get_wireguard_node_id() {
                node_id.to_string()
            } else {
                conn.get_env()
            };

            let _ = publisher.send_binary(&topic, bytes.as_ref()).await;

            return Ok(http::success_response(
                format!("Connection {} has been created", id),
                Some(id),
                http::Instance::Connection(conn),
            ));
        }

        Ok(StorageOperationStatus::AlreadyExist(id)) => Ok(http::not_modified(&format!(
            "Connection {} already exists",
            id
        ))),

        Ok(StorageOperationStatus::BadRequest(id, msg)) => {
            Ok(http::bad_request(&format!("BadRequest {} {}", id, msg)))
        }

        Ok(status) => Ok(http::bad_request(&format!(
            "Unsupported operation status: {}",
            status
        ))),

        Err(err) => Ok(http::internal_error(&format!(
            "Internal error while processing connection {}: {}",
            conn_id, err
        ))),
    }
}

/// Handler deletes connection
// DELETE /connection?id=
pub async fn delete_connection_handler<N, C, S>(
    conn_param: ConnQueryParam,
    publisher: ZmqPublisher,
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
    let conn_id = conn_param.id;
    let conn_opt = {
        let mem = memory.memory.read().await;
        mem.connections.get(&conn_id).cloned()
    };

    let Some(conn) = conn_opt else {
        return Ok(http::not_found(&format!(
            "Connection {} not found",
            conn_id
        )));
    };

    if conn.get_deleted() {
        return Ok(http::not_found(&format!(
            "Connection {} is deleted",
            conn_id
        )));
    }

    match SyncOp::delete_connection(&memory, &conn_id).await {
        Ok(StorageOperationStatus::Ok(id)) => {
            let msg = conn.as_delete_message(&conn_id);

            let bytes = match rkyv::to_bytes::<_, 1024>(&msg) {
                Ok(b) => b,
                Err(e) => {
                    return Ok(http::internal_error(&format!("Serialization error: {}", e)));
                }
            };

            if let Some(node_id) = conn.get_wireguard_node_id() {
                let _ = publisher
                    .send_binary(&node_id.to_string(), bytes.as_ref())
                    .await;
            } else {
                let _ = publisher.send_binary(&conn.get_env(), bytes.as_ref()).await;
            }

            Ok(http::success_response(
                format!("Connection {} has been deleted", id),
                Some(id),
                http::Instance::Connection(conn.clone().into()),
            ))
        }

        Ok(StorageOperationStatus::NotFound(id)) => {
            Ok(http::not_found(&format!("Connection {} not found", id)))
        }

        Ok(status) => Ok(http::bad_request(&format!(
            "Unsupported operation status: {}",
            status
        ))),

        Err(err) => Ok(http::internal_error(&format!(
            "Internal error while deleting connection {}: {}",
            conn_id, err
        ))),
    }
}

/// Handler updates connection
// PUT /connection?id=
pub async fn put_connection_handler<N, C, S>(
    conn_param: ConnQueryParam,
    conn_req: ConnUpdateRequest,
    publisher: ZmqPublisher,
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
    let conn_id = conn_param.id;
    log::debug!("Connection to update {}", conn_id);

    match SyncOp::update_conn(&memory, &conn_id, conn_req).await {
        Ok(StorageOperationStatus::Updated(id)) => {
            let mem = memory.memory.read().await;

            if let Some(conn) = mem.connections.get(&id) {
                let msg = conn.as_update_message(&id);

                let bytes = match rkyv::to_bytes::<_, 1024>(&msg) {
                    Ok(b) => b,
                    Err(e) => {
                        return Ok(http::internal_error(&format!("Serialization error: {}", e)));
                    }
                };

                let _ = publisher.send_binary(&conn.get_env(), bytes.as_ref()).await;

                Ok(http::success_response(
                    format!("Connection {} has been updated", id),
                    Some(id),
                    http::Instance::None,
                ))
            } else {
                Ok(http::not_found(&format!("Connection {} is not found", id)))
            }
        }
        Ok(StorageOperationStatus::NotModified(id)) => Ok(http::not_modified(&format!(
            "Connection {} is not modified",
            id
        ))),
        Ok(StorageOperationStatus::NotFound(id)) => {
            Ok(http::not_found(&format!("Connection {} is not found", id)))
        }
        Ok(StorageOperationStatus::BadRequest(id, msg)) => {
            Ok(http::bad_request(&format!("BadRequest {} {}", id, msg)))
        }
        Ok(status) => Ok(http::internal_error(&format!(
            "Unsupported operation status: {}",
            status
        ))),
        Err(err) => Ok(http::internal_error(&format!(
            "Internal error while processing connection {}: {}",
            conn_id, err
        ))),
    }
}

/// Get connection detaisl
// GET /connection?id=
pub async fn get_connection_handler<N, C, S>(
    conn_param: ConnQueryParam,
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
        + PartialEq
        + serde::ser::Serialize,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
    Connection: From<C>,
{
    let mem = memory.memory.read().await;

    let conn_id = conn_param.id;

    if let Some(conn) = mem.connections.get(&conn_id) {
        return Ok(http::success_response(
            "Connection is found".to_string(),
            Some(conn_id),
            http::Instance::Connection(conn.clone().into()),
        ));
    } else {
        return Ok(http::not_found("Connection is not found"));
    }
}
