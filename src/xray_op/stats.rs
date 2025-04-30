use async_trait::async_trait;
use tonic::Status;

use crate::state::stats::ConnStat;
use crate::state::stats::InboundStat;
use crate::state::stats::Stat;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;
use crate::xray_op::Tag;

#[derive(Debug, Clone)]
pub enum Prefix {
    ConnPrefix(uuid::Uuid),
    InboundPrefix(Tag),
}

impl Prefix {
    pub fn as_tag(&self) -> Option<&Tag> {
        if let Prefix::InboundPrefix(tag) = self {
            Some(tag)
        } else {
            None
        }
    }
}

#[async_trait]
pub trait StatOp {
    async fn stat(
        &self,
        prefix: Prefix,
        stat_type: Stat,
        reset: bool,
    ) -> Result<GetStatsResponse, Status>;
    async fn conn_stats(&self, conn_id: Prefix) -> Result<ConnStat, Status>;
    async fn inbound_stats(&self, inbound: Prefix) -> Result<InboundStat, Status>;
    async fn conn_count(&self, inbound: Tag) -> Result<Option<i64>, Status>;
}
