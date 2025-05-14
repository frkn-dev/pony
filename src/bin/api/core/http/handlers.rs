use base64::Engine;
use chrono::Utc;
use std::collections::HashSet;
use warp::http::StatusCode;

use pony::http::requests::ConnQueryParam;
use pony::http::requests::UserSubQueryParam;

use pony::http::requests::ConnRequest;
use pony::http::requests::NodeRequest;
use pony::http::requests::NodeResponse;
use pony::http::requests::NodesQueryParams;
use pony::http::requests::UserIdQueryParam;
use pony::http::ResponseMessage;
use pony::state::Conn;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::ConnStat;
use pony::state::ConnStatus;
use pony::state::ConnStorageApi;
use pony::state::ConnStorageBase;
use pony::state::ConnStorageOpStatus;
use pony::state::NodeStatus;
use pony::state::NodeStorage;
use pony::state::NodeStorageOpStatus;
use pony::state::SyncOp;
use pony::state::SyncState;
use pony::state::Tag;
use pony::state::User;
use pony::state::UserStorage;
use pony::state::UserStorageOpStatus;
use pony::utils;
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::zmq::publisher::Publisher as ZmqPublisher;

pub async fn healthcheck_handler<T, C>(
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let mem = state.memory.lock().await;

    if mem.connections.len() > 0 {
        let response = ResponseMessage::<String> {
            status: 200,
            message: "OK".to_string(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 404,
            message: "Connections not found".to_string(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}

pub async fn get_users_handler<T, C>(
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let mem = state.memory.lock().await;
    let users = mem.users.all();

    if let Ok(users) = users {
        let users = users
            .into_iter()
            .filter(|(_, user)| !user.is_deleted)
            .collect();
        let response = ResponseMessage::<Vec<(uuid::Uuid, User)>> {
            status: 200,
            message: users,
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 404,
            message: "Users not found".to_string(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}

pub async fn user_register_handler<T, C>(
    user_req: User,
    state: SyncState<T, C>,
    publisher: ZmqPublisher,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Received: {:?}", user_req);

    let user_id = uuid::Uuid::new_v4();
    let user = User::new(
        user_req.username,
        user_req.telegram_id,
        &user_req.env,
        user_req.limit,
        user_req.password.clone(),
    );

    match SyncOp::add_user(&state, &user_id, user.clone()).await {
        Ok(UserStorageOpStatus::Ok) => {
            let response = ResponseMessage::<uuid::Uuid> {
                status: 200,
                message: user_id,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Ok(UserStorageOpStatus::AlreadyExist) => {
            let response = ResponseMessage::<String> {
                status: 304,
                message: format!("User {} is already exist", user_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_MODIFIED,
            ))
        }
        Ok(UserStorageOpStatus::Updated) => {
            let conns = {
                let mem = state.memory.lock().await;
                if let Some(user_id) = mem.users.by_username(&user.username) {
                    mem.connections.get_by_user_id(&user_id)
                } else {
                    None
                }
            };

            if let Some(conns) = conns {
                for (conn_id, conn) in conns {
                    let now = Utc::now();
                    let new_conn = Conn {
                        trial: conn.get_trial(),
                        limit: conn.get_limit(),
                        env: conn.get_env(),
                        status: conn.get_status(),
                        stat: ConnStat::default(),
                        created_at: now,
                        modified_at: now,
                        proto: conn.get_proto(),
                        password: conn.get_password(),
                        user_id: conn.get_user_id(),
                        is_deleted: false,
                    };
                    let _ = SyncOp::add_or_update_conn(&state, &conn_id, new_conn).await;

                    let msg = Message {
                        action: Action::Create,
                        conn_id: conn_id,
                        password: user.password.clone(),
                        tag: conn.get_proto(),
                    };

                    let _ = publisher.send(&user.env, msg).await;
                }
            }

            let response = ResponseMessage::<uuid::Uuid> {
                status: 200,
                message: user_id,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::ACCEPTED,
            ))
        }
        Err(e) => {
            log::error!("{}", e);
            let response = ResponseMessage::<String> {
                status: 500,
                message: format!("Error: {}", e),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub async fn user_conn_stat_handler<T, C>(
    user_req: UserIdQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Received: {:?}", user_req);

    let mem = state.memory.lock().await;

    let mut result: Vec<(uuid::Uuid, ConnStat, Tag, ConnStatus)> = vec![];

    if let Some(connections) = mem.connections.get_by_user_id(&user_req.user_id) {
        for (conn_id, conn) in connections {
            let stat = ConnStat {
                online: conn.get_online(),
                downlink: conn.get_downlink(),
                uplink: conn.get_uplink(),
            };

            result.push((conn_id, stat, conn.get_proto(), conn.get_status()));
        }

        let response = ResponseMessage::<Vec<(uuid::Uuid, ConnStat, Tag, ConnStatus)>> {
            status: 200,
            message: result,
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<String> {
            status: 404,
            message: "Connections not found".to_string(),
        };

        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    }
}

// Register node handler
pub async fn node_register_handler<T, C>(
    node_req: NodeRequest,
    state: SyncState<T, C>,
    publisher: ZmqPublisher,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let node_id = node_req.uuid;

    match SyncOp::add_node(&state, &node_id, node.clone()).await {
        Ok(status @ (NodeStorageOpStatus::Ok | NodeStorageOpStatus::AlreadyExist)) => {
            if status == NodeStorageOpStatus::AlreadyExist {
                let _ = SyncOp::update_node_status(&state, &node_id, &node.env, NodeStatus::Online)
                    .await;
            }

            let mem = state.memory.lock().await;
            for (conn_id, conn) in mem
                .connections
                .iter()
                .filter(|(_, conn)| !conn.get_deleted() && conn.get_status() == ConnStatus::Active)
            {
                let msg = Message {
                    conn_id: *conn_id,
                    action: Action::Create,
                    password: conn.get_password(),
                    tag: conn.get_proto(),
                };
                let _ = publisher.send(&node_req.uuid.to_string(), msg).await;
            }

            let (status_code, message) = match status {
                NodeStorageOpStatus::Ok => (StatusCode::OK, format!("node {} is added", node_id)),
                NodeStorageOpStatus::AlreadyExist => (
                    StatusCode::NOT_MODIFIED,
                    format!("node {} already exists", node_id),
                ),
            };

            let response = ResponseMessage::<String> {
                status: status_code.as_u16(),
                message,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                status_code,
            ))
        }

        Err(e) => {
            let response = ResponseMessage::<String> {
                status: 500,
                message: format!("INTERNAL_SERVER_ERROR {} for {}", e, node_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

/// Handler creates connection
pub async fn create_or_update_connection_handler<T, C>(
    conn_req: ConnRequest,
    publisher: ZmqPublisher,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn>,
{
    let conn = Conn::new(
        conn_req.trial.unwrap_or(true),
        conn_req.limit.unwrap_or(0),
        &conn_req.env,
        ConnStatus::Active,
        conn_req.password.clone(),
        conn_req.user_id,
        ConnStat::default(),
        conn_req.proto,
    )
    .into();

    let msg = conn_req.as_message();

    match SyncOp::add_or_update_conn(&state, &conn_req.conn_id, conn).await {
        Ok(ConnStorageOpStatus::Ok) => {
            log::debug!("/connection req Message sent {}", msg);
            let _ = publisher.send(&conn_req.env.to_string(), msg.clone()).await;
            let response = ResponseMessage {
                status: 200,
                message: format!("connection {} is added", conn_req.conn_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Ok(ConnStorageOpStatus::Updated) => {
            log::debug!("/connection req Message sent {}", msg);
            let _ = publisher.send(&conn_req.env.to_string(), msg.clone()).await;
            let response = ResponseMessage {
                status: 200,
                message: format!("connection {} is modified", conn_req.conn_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        Ok(ConnStorageOpStatus::AlreadyExist) => {
            let response = ResponseMessage {
                status: 304,
                message: format!("connection {} is modified", conn_req.conn_id),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_MODIFIED,
            ))
        }
        Ok(op) => {
            let error_message = format!("Error: Operation isn't supported: {}", op);
            let json_error_message = warp::reply::json(&error_message);
            Ok(warp::reply::with_status(
                json_error_message,
                StatusCode::BAD_REQUEST,
            ))
        }
        Err(err) => {
            let error_message = format!("Error: Cannot handle /connection req: {}", err);
            let json_error_message = warp::reply::json(&error_message);
            Ok(warp::reply::with_status(
                json_error_message,
                StatusCode::BAD_REQUEST,
            ))
        }
    }
}

/// List of nodes handler
pub async fn get_nodes_handler<T, C>(
    node_req: NodesQueryParams,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static,
{
    let state = state.memory.lock().await;

    let nodes = state.nodes.get_by_env(&node_req.env);

    match nodes {
        Some(nodes) => {
            let node_response: Vec<NodeResponse> = nodes
                .into_iter()
                .map(|node| node.as_node_response())
                .collect();
            let response = ResponseMessage::<Vec<NodeResponse>> {
                status: 200,
                message: node_response,
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        None => {
            let response = ResponseMessage::<String> {
                status: 404,
                message: "Env not found".to_string(),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ))
        }
    }
}

/// Get list of user connection credentials
pub async fn get_user_connections_handler<T, C>(
    user_req: UserIdQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp
        + ConnApiOp
        + Sync
        + Send
        + Clone
        + 'static
        + From<Conn>
        + std::fmt::Debug
        + serde::ser::Serialize,
{
    log::debug!("Received: {:?}", user_req);

    let user_id = user_req.user_id;

    let connections = {
        let mem = state.memory.lock().await;
        mem.connections.get_by_user_id(&user_id)
    };

    if connections.is_none() {
        log::debug!("Connections not found");

        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "NO CONNECTIONS AVAILABLE" })),
            warp::http::StatusCode::NOT_FOUND,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&connections),
        warp::http::StatusCode::OK,
    ))
}

/// Get list of user connection credentials
pub async fn connections_lines_handler<T, C>(
    conn_req: ConnQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn> + std::fmt::Debug,
{
    let state = state.memory.lock().await;

    let conn_id = conn_req.conn_id;
    let mut connection_links = Vec::new();

    if let Some(conn) = state.connections.get(&conn_id) {
        if let Some(nodes) = state.nodes.get_by_env(&conn.get_env()) {
            for node in nodes
                .iter()
                .filter(|node| node.status == NodeStatus::Online)
            {
                for (tag, inbound) in &node.inbounds {
                    let link = utils::create_conn_link(
                        *tag,
                        &conn_id,
                        inbound.as_inbound_response(),
                        &node.label,
                        node.address,
                    );

                    if let Ok(link) = link {
                        connection_links.push(link);
                    }
                }
            }
        }
    }

    if connection_links.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "NO CONNECTIONS AVAILABLE" })),
            warp::http::StatusCode::NOT_FOUND,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&connection_links),
        warp::http::StatusCode::OK,
    ))
}

pub async fn subscription_link_handler<T, C>(
    user_req: UserSubQueryParam,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn> + std::fmt::Debug,
{
    let mem = state.memory.lock().await;

    let conns = mem.connections.get_by_user_id(&user_req.user_id);
    let mut inbounds_by_node = vec![];

    if let Some(conns) = conns {
        for (conn_id, conn) in conns {
            if conn.get_deleted() || conn.get_status() != ConnStatus::Active {
                continue;
            }
            if let Some(nodes) = mem.nodes.get_by_env(&conn.get_env()) {
                for node in nodes.iter().filter(|n| n.status == NodeStatus::Online) {
                    if let Some(inbound) = &node.inbounds.get(&conn.get_proto()) {
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
        return Ok(warp::reply::with_status(
            warp::reply::with_header(
                "NO SUBSCRIPTION LINKS AVAILABLE".to_string(),
                "Content-Type",
                "application/yaml",
            ),
            StatusCode::NOT_FOUND,
        ));
    }

    match user_req.format.as_str() {
        "clash" => {
            let mut proxies = vec![];

            for (inbound, conn_id, label, address) in &inbounds_by_node {
                if let Some(proxy) =
                    utils::generate_proxy_config(inbound, *conn_id, *address, label)
                {
                    proxies.push(proxy)
                }
            }

            let config = utils::generate_clash_config(proxies);
            let yaml = serde_yaml::to_string(&config)
                .unwrap_or_else(|_| "---\nerror: failed to serialize\n".into());

            return Ok(warp::reply::with_status(
                warp::reply::with_header(yaml, "Content-Type", "application/yaml"),
                StatusCode::OK,
            ));
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

            return Ok(warp::reply::with_status(
                warp::reply::with_header(body, "Content-Type", "text/plain"),
                StatusCode::OK,
            ));
        }
    }
}

pub async fn create_all_connections_handler<T, C>(
    user_req: UserIdQueryParam,
    publisher: ZmqPublisher,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Run create_all_connections_handler");

    // Lock and extract required data without holding the lock across await
    let (user, nodes, env, existing_protos) = {
        let mem = state.memory.lock().await;

        let Some(user) = mem.users.get(&user_req.user_id) else {
            return Ok(warp::reply::with_status(
                warp::reply::json(
                    &serde_json::json!({ "status": "error", "message": "user not found" }),
                ),
                warp::http::StatusCode::NOT_FOUND,
            ));
        };

        let env = user.env.clone();

        let existing_protos: HashSet<_> = mem
            .connections
            .get_by_user_id(&user_req.user_id)
            .map(|c| c.iter().map(|(_, conn)| conn.get_proto()).collect())
            .unwrap_or_default();

        let nodes = match mem.nodes.get_by_env(&env) {
            Some(n) => n.clone(),
            None => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(
                        &serde_json::json!({ "status": "error", "message": "nodes not found" }),
                    ),
                    warp::http::StatusCode::NOT_FOUND,
                ));
            }
        };

        (user.clone(), nodes, env, existing_protos)
    };

    let mut available_protos = HashSet::new();
    for node in nodes.iter().filter(|n| n.status == NodeStatus::Online) {
        for tag in node.inbounds.keys() {
            available_protos.insert(*tag);
        }
    }

    for tag in available_protos.difference(&existing_protos) {
        let conn_id = uuid::Uuid::new_v4();
        let conn = Conn::new(
            true,
            user.limit.unwrap_or(0),
            &env,
            ConnStatus::Active,
            None,
            Some(user_req.user_id),
            ConnStat::default(),
            *tag,
        );

        let msg = Message {
            action: Action::Create,
            conn_id,
            password: user.password.clone(),
            tag: conn.get_proto(),
        };

        match SyncOp::add_or_update_conn(&state, &conn_id, conn.clone()).await {
            Ok(ConnStorageOpStatus::Ok | ConnStorageOpStatus::Updated) => {
                log::debug!("/connection req Message sent {}", msg);
                let _ = publisher.send(&conn.get_env(), msg.clone()).await;
            }
            _ => {
                log::warn!("Failed to add conn {} for tag {:?}", conn_id, tag);
            }
        };
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "Ok" })),
        warp::http::StatusCode::OK,
    ))
}

pub async fn delete_user_handler<T, C>(
    user_req: UserIdQueryParam,
    publisher: ZmqPublisher,
    state: SyncState<T, C>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    T: NodeStorage + Sync + Send + Clone + 'static,
    C: ConnBaseOp + ConnApiOp + Sync + Send + Clone + 'static + From<Conn>,
{
    log::debug!("Run delete_user_handler");

    // Lock and extract required data without holding the lock across await
    let (user, conns) = {
        let mem = state.memory.lock().await;

        let Some(user) = mem.users.get(&user_req.user_id) else {
            return Ok(warp::reply::with_status(
                warp::reply::json(
                    &serde_json::json!({ "status": "error", "message": "user not found" }),
                ),
                warp::http::StatusCode::NOT_FOUND,
            ));
        };

        let conns = mem.connections.get_by_user_id(&user_req.user_id);

        (user.clone(), conns)
    };

    match SyncOp::delete_user(&state, &user_req.user_id).await {
        Ok(_) => {
            if let Some(conns) = conns {
                for (conn_id, conn) in conns {
                    let msg = Message {
                        action: Action::Delete,
                        conn_id,
                        password: user.password.clone(),
                        tag: conn.get_proto(),
                    };
                    if let Err(e) = SyncOp::delete_connection(&state, &conn_id).await {
                        log::error!("Cannot delete connection {}", e);
                    }
                    let _ = publisher.send(&conn.get_env(), msg.clone()).await;
                }
            }
            return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({ "status": "Ok" })),
                warp::http::StatusCode::OK,
            ));
        }
        _ => {
            log::warn!("Failed delete user {} ", user_req.user_id);
            return Ok(warp::reply::with_status(
                warp::reply::json(
                    &serde_json::json!({ "status": "error", "message": "fail to delete user" }),
                ),
                warp::http::StatusCode::NOT_MODIFIED,
            ));
        }
    }
}
