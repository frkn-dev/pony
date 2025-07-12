use pony::http::requests::NodeIdParam;
use pony::http::requests::NodeTypeParam;
use pony::http::IdResponse;
use pony::Tag;
use rkyv::to_bytes;
use warp::http::StatusCode;

use pony::http::requests::NodeRequest;
use pony::http::requests::NodeResponse;
use pony::http::requests::NodeType;
use pony::http::requests::NodesQueryParams;
use pony::http::MyRejection;
use pony::http::ResponseMessage;
use pony::memory::node::Status as NodeStatus;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::ConnectionStatus;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::Publisher as ZmqPublisher;

use crate::core::clickhouse::score::NodeScore;
use crate::core::clickhouse::ChContext;
use crate::core::sync::tasks::SyncOp;
use crate::core::sync::MemSync;

// Register node handler
// POST /node
pub async fn post_node_handler<N, C>(
    node_req: NodeRequest,
    node_param: NodeTypeParam,
    memory: MemSync<N, C>,
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

    let node_type = node_param.node_type.unwrap_or(NodeType::All);

    let status = SyncOp::add_node(&memory, &node_id, node.clone()).await;

    let result_response = match status {
        Ok(StorageOperationStatus::Ok(id))
        | Ok(StorageOperationStatus::AlreadyExist(id))
        | Ok(StorageOperationStatus::NotModified(id)) => {
            let _ =
                SyncOp::update_node_status(&memory, &node_id, &node.env, NodeStatus::Online).await;

            let mem = memory.memory.read().await;

            let connections_iter = mem.connections.iter().filter(|(_, conn)| {
                !conn.get_deleted()
                    && conn.get_status() == ConnectionStatus::Active
                    && match node_type {
                        NodeType::Wireguard => {
                            log::debug!("NODE TYPE {:?} {:?}", node_type, conn.get_proto().proto());
                            conn.get_proto().proto() == Tag::Wireguard
                                && Some(node_id) == conn.get_wireguard_node_id()
                        }
                        NodeType::Xray => conn.get_proto().proto() != Tag::Wireguard,
                        NodeType::All => match conn.get_proto().proto() {
                            Tag::Wireguard => conn.get_wireguard_node_id() == Some(node_id),
                            _ => true,
                        },
                    }
            });

            for (conn_id, conn) in connections_iter {
                let message = conn.as_create_message(conn_id);

                let bytes = to_bytes::<_, 1024>(&message)
                    .map_err(|e| warp::reject::custom(MyRejection(Box::new(e))))?;

                publisher
                    .send_binary(&node_req.uuid.to_string(), bytes.as_ref())
                    .await
                    .map_err(|e| warp::reject::custom(MyRejection(Box::new(e))))?;
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
        warp::reply::json(&result_response),
        StatusCode::from_u16(result_response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
    ))
}

/// List of nodes handler
pub async fn get_nodes_handler<N, C>(
    node_req: NodesQueryParams,
    memory: MemSync<N, C>,
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
    let mem = memory.memory.read().await;

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
// GET /node?node_id=
pub async fn get_node_handler<N, C>(
    node_req: NodeIdParam,
    memory: MemSync<N, C>,
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
    let mem = memory.memory.read().await;

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

/// Get score of load a node handler
// GET /node/score?node_id=
pub async fn get_node_score_handler<N, C>(
    node_req: NodeIdParam,
    memory: MemSync<N, C>,
    ch: ChContext,
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
    let mem = memory.memory.read().await;

    if let Some(node) = mem.nodes.get_by_id(&node_req.node_id) {
        let interface = node.interface.clone();
        let env = node.env.clone();
        let hostname = node.hostname.clone();
        let cores = node.cores;
        let max_bandwidth_bps = node.max_bandwidth_bps;

        if let Some(score) = ch
            .fetch_node_score(&env, &hostname, &interface, cores, max_bandwidth_bps)
            .await
        {
            let response = ResponseMessage::<Option<NodeScore>> {
                status: StatusCode::OK.as_u16(),
                message: format!("Node score for {}", node_req.node_id),
                response: Some(score),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        } else {
            let response = ResponseMessage::<Option<NodeScore>> {
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                message: "Failed to calculate node score".to_string(),
                response: None,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    } else {
        let response = ResponseMessage::<Option<NodeScore>> {
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
