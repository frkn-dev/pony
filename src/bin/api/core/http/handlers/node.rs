use warp::http::StatusCode;

use pony::http::requests::NodeIdParam;
use pony::http::requests::NodeRequest;
use pony::http::requests::NodeResponse;
use pony::http::requests::NodesQueryParams;
use pony::http::IdResponse;
use pony::http::ResponseMessage;
use pony::memory::node::Status as NodeStatus;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::OperationStatus as StorageOperationStatus;
use pony::SubscriptionOp;

use crate::core::clickhouse::score::NodeScore;
use crate::core::clickhouse::ChContext;
use crate::core::sync::tasks::SyncOp;
use crate::core::sync::MemSync;

// Register node handler
// POST /node
pub async fn post_node_handler<N, C, S>(
    node_req: NodeRequest,
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
    log::debug!("Received node request: {:?}", node_req);

    let node = node_req.clone().as_node();
    let node_id = node_req.uuid;

    let status = SyncOp::add_node(&memory, &node_id, node.clone()).await;

    let result_response = match status {
        Ok(StorageOperationStatus::Ok(id))
        | Ok(StorageOperationStatus::AlreadyExist(id))
        | Ok(StorageOperationStatus::NotModified(id)) => {
            let _ =
                SyncOp::update_node_status(&memory, &node_id, &node.env, NodeStatus::Online).await;

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
            log::error!("Error adding node: {} for {}", e, node_id);
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

    let status_code =
        StatusCode::from_u16(result_response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Ok(warp::reply::with_status(
        warp::reply::json(&result_response),
        status_code,
    ))
}

/// List of nodes handler
pub async fn get_nodes_handler<N, C, S>(
    node_param: NodesQueryParams,
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
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    let mem = memory.memory.read().await;

    let nodes = if let Some(env) = node_param.env {
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
// GET /node?id=
pub async fn get_node_handler<N, C, S>(
    node_param: NodeIdParam,
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
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    let mem = memory.memory.read().await;

    if let Some(node) = mem.nodes.get_by_id(&node_param.id) {
        let response = ResponseMessage::<Option<NodeResponse>> {
            status: StatusCode::OK.as_u16(),
            message: format!("Node {}", node_param.id),
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
// GET /node/score?id=
pub async fn get_node_score_handler<N, C, S>(
    node_param: NodeIdParam,
    memory: MemSync<N, C, S>,
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
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    let mem = memory.memory.read().await;

    if let Some(node) = mem.nodes.get_by_id(&node_param.id) {
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
                message: format!("Node score for {}", node_param.id),
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
