use async_trait::async_trait;
use tonic::Status;

use crate::memory::connection::stat::Stat as ConnectionStat;
use crate::memory::node::Stat as InboundStat;
use crate::memory::stat::Stat;
use crate::memory::tag::ProtoTag as Tag;
use crate::xray_api::xray::app::stats::command::GetStatsResponse;

#[derive(Debug, Clone, Copy)]
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
        stat_kind: Stat,
        reset: bool,
    ) -> Result<GetStatsResponse, Status>;
    async fn conn_stats(&self, conn_id: Prefix) -> Result<ConnectionStat, Status>;
    async fn inbound_stats(&self, inbound: Prefix) -> Result<InboundStat, Status>;
    async fn conn_count(&self, inbound: Tag) -> Result<Option<i64>, Status>;
    async fn reset_stat(&self, conn_id: &uuid::Uuid) -> Result<(), Status>;
}
