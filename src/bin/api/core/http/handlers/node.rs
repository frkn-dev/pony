use pony::http::requests::NodeIdParam;
use pony::http::requests::NodeTypeParam;
use pony::http::IdResponse;
use pony::Tag;
use warp::http::StatusCode;

use pony::http::requests::NodeRequest;
use pony::http::requests::NodeResponse;
use pony::http::requests::NodeType;
use pony::http::requests::NodesQueryParams;
use pony::http::ResponseMessage;
use pony::state::node::Status as NodeStatus;
use pony::zmq::message::Action;
use pony::zmq::message::Message;
use pony::zmq::publisher::Publisher as ZmqPublisher;
use pony::Conn as Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStatus;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;

use crate::core::sync::tasks::SyncOp;
use crate::core::sync::SyncState;

// Register node handler
// POST /node
pub async fn post_node_handler<N, C>(
    node_req: NodeRequest,
    node_param: NodeTypeParam,
    state: SyncState<N, C>,
    publisher: ZmqPublisher,
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
    log::debug!("Received: {:?}", node_req);

    let node = node_req.clone().as_node();
    let node_id = node_req.uuid;

    let node_type = if let Some(node_type) = node_param.node_type {
        node_type
    } else {
        NodeType::All
    };

    let status = SyncOp::add_node(&state, &node_id, node.clone()).await;

    let response = match status {
        Ok(StorageOperationStatus::Ok(id))
        | Ok(StorageOperationStatus::AlreadyExist(id))
        | Ok(StorageOperationStatus::NotModified(id)) => {
            let _ =
                SyncOp::update_node_status(&state, &node_id, &node.env, NodeStatus::Online).await;

            let mem = state.memory.lock().await;

            let connections_iter = mem.connections.iter().filter(|(_, conn)| {
                !conn.get_deleted()
                    && conn.get_status() == ConnectionStatus::Active
                    && match node_type {
                        NodeType::Wireguard => {
                            conn.get_proto().proto() == Tag::Wireguard
                                && Some(node_id) == conn.get_wireguard_node_id()
                        }
                        NodeType::Xray => conn.get_proto().proto() != Tag::Wireguard,
                        NodeType::All => {
                            if conn.get_proto().proto() == Tag::Wireguard {
                                Some(node_id) == conn.get_wireguard_node_id()
                            } else {
                                true
                            }
                        }
                    }
            });

            for (conn_id, conn) in connections_iter {
                let message = Message {
                    conn_id: *conn_id,
                    action: Action::Create,
                    password: conn.get_password(),
                    tag: conn.get_proto().proto(),
                    wg: conn.get_wireguard().cloned(),
                };

                if let Err(e) = publisher.send(&node_req.uuid.to_string(), message).await {
                    log::error!("Failed to send message for connection {}: {}", conn_id, e);
                }
            }

            ResponseMessage::<Option<IdResponse>> {
                status: StatusCode::OK.as_u16(),
                message: "Ok".to_string(),
                response: Some(IdResponse { id }),
            }
        }

        Ok(StorageOperationStatus::Updated(_))
        | Ok(StorageOperationStatus::DeletedPreviously(_))
        | Ok(StorageOperationStatus::BadRequest(_, _))
        | Ok(StorageOperationStatus::UpdatedStat(_))
        | Ok(StorageOperationStatus::NotFound(_)) => ResponseMessage::<Option<IdResponse>> {
            status: StatusCode::BAD_REQUEST.as_u16(),
            message: "Operation not supported".into(),
            response: None,
        },

        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ResponseMessage::<Option<uuid::Uuid>> {
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    message: format!("INTERNAL_SERVER_ERROR {} for {}", e, node_id),
                    response: None,
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
    ))
}

/// List of nodes handler
pub async fn get_nodes_handler<N, C>(
    node_req: NodesQueryParams,
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
{
    let mem = state.memory.lock().await;

    let nodes = if let Some(env) = node_req.env {
        mem.nodes.get_by_env(&env)
    } else {
        mem.nodes.all()
    };

    match nodes {
        Some(nodes) => {
            let node_response: Vec<NodeResponse> = nodes
                .into_iter()
                .map(|node| node.as_node_response())
                .collect();
            let response = ResponseMessage::<Option<Vec<NodeResponse>>> {
                status: StatusCode::OK.as_u16(),
                message: "List of nodes".to_string(),
                response: Some(node_response),
            };

            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        }
        None => {
            let response = ResponseMessage::<Option<Vec<NodeResponse>>> {
                status: StatusCode::NOT_FOUND.as_u16(),
                message: "List of nodes".to_string(),
                response: vec![].into(),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::NOT_FOUND,
            ))
        }
    }
}

/// Get of a node handler
pub async fn get_node_handler<N, C>(
    node_req: NodeIdParam,
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
{
    let mem = state.memory.lock().await;

    if let Some(node) = mem.nodes.get_by_id(&node_req.node_id) {
        let response = ResponseMessage::<Option<NodeResponse>> {
            status: StatusCode::OK.as_u16(),
            message: format!("Node {}", node_req.node_id),
            response: Some(node.as_node_response()),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        ))
    } else {
        let response = ResponseMessage::<Option<NodeResponse>> {
            status: StatusCode::NOT_FOUND.as_u16(),
            message: "Node not found".to_string(),
            response: None,
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::NOT_FOUND,
        ))
    }
}
