use async_trait::async_trait;
use tonic::{Request, Status};

use crate::xray_api::xray::app::proxyman::command::{
    GetInboundUserRequest, GetInboundUserResponse, GetInboundUsersCountResponse,
};

use super::client::HandlerClient;
use crate::state::tag::Tag;

#[async_trait]
pub trait ConnOp {
    async fn conn_op(
        &mut self,
        tag: Tag,
        user_id: String,
    ) -> Result<GetInboundUserResponse, Status>;
    async fn conn_count_op(&mut self, tag: Tag) -> Result<i64, Status>;
}

#[async_trait]
impl ConnOp for HandlerClient {
    /// Not used
    async fn conn_op(
        &mut self,
        tag: Tag,
        conn_id: String,
    ) -> Result<GetInboundUserResponse, Status> {
        let request = GetInboundUserRequest {
            tag: tag.to_string(),
            email: format!("{}@pony", conn_id).to_string(),
        };

        self.client
            .get_inbound_users(Request::new(request))
            .await
            .map(|res| res.into_inner())
            .map_err(|e| {
                log::error!("Failed to fetch conn for tag {}: {}", tag, e);
                e
            })
    }

    async fn conn_count_op(&mut self, tag: Tag) -> Result<i64, Status> {
        let request = GetInboundUserRequest {
            tag: tag.to_string(),
            email: "".to_string(),
        };

        match self
            .client
            .get_inbound_users_count(Request::new(request))
            .await
        {
            Ok(res) => {
                let res: GetInboundUsersCountResponse = res.into_inner();
                Ok(res.count)
            }
            Err(e) => {
                let error = format!("Failed to fetch conn count for tag {}: {}", tag, e);
                Err(Status::internal(error))
            }
        }
    }
}
