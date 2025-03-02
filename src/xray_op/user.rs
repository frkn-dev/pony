use log::error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Status};

use crate::xray_api::xray::app::proxyman::command::{
    GetInboundUserRequest, GetInboundUserResponse, GetInboundUsersCountResponse,
};

use crate::state::tag::Tag;

use super::client::HandlerClient;

pub async fn get_user(
    client: Arc<Mutex<HandlerClient>>,
    tag: Tag,
    user_id: String,
) -> Result<GetInboundUserResponse, Status> {
    let request = GetInboundUserRequest {
        tag: tag.to_string(),
        email: format!("{}@pony", user_id).to_string(),
    };

    let mut handler_client = client.lock().await;

    handler_client
        .client
        .get_inbound_users(Request::new(request))
        .await
        .map(|res| res.into_inner())
        .map_err(|e| {
            error!("Failed to fetch users for tag {}: {}", tag, e);
            e
        })
}

pub async fn user_count(client: Arc<Mutex<HandlerClient>>, tag: Tag) -> Result<i64, Status> {
    let request = GetInboundUserRequest {
        tag: tag.to_string(),
        email: "".to_string(),
    };

    let mut handler_client = client.lock().await;

    match handler_client
        .client
        .get_inbound_users_count(Request::new(request))
        .await
    {
        Ok(res) => {
            let res: GetInboundUsersCountResponse = res.into_inner();
            Ok(res.count)
        }
        Err(e) => {
            let error = format!("Failed to fetch users for tag {}: {}", tag, e);
            Err(Status::internal(error))
        }
    }
}
