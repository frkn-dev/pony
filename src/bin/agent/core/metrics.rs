use chrono::Utc;

use crate::core::Agent;
use pony::metrics::storage::HasMetrics;
use pony::metrics::storage::MetricSink;
use pony::metrics::storage::MetricStorage;

use pony::ConnectionBaseOp;

impl<C> HasMetrics for Agent<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    fn metrics(&self) -> &MetricStorage {
        &self.metrics
    }

    fn node_id(&self) -> &uuid::Uuid {
        &self.node.uuid
    }
}

pub trait BusinessMetrics {
    async fn inbounds(&self);
    async fn connections(&self);
}

impl<C> BusinessMetrics for Agent<C>
where
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
{
    async fn connections(&self) {
        let now = Utc::now().timestamp();
        let connections = self.memory.read().await.clone();
        let node_id = self.node_id();
        for (conn_id, conn) in connections.iter() {
            let proto = conn.get_proto().proto();
            let base = format!("connection.{}.{}", proto, conn_id);
            self.metrics.write(
                node_id,
                &format!("{base}.uplink"),
                conn.get_uplink() as f64,
                now,
            );
            self.metrics.write(
                node_id,
                &format!("{base}.downlink"),
                conn.get_downlink() as f64,
                now,
            );
            self.metrics.write(
                node_id,
                &format!("{base}.online"),
                conn.get_online() as f64,
                now,
            );
            log::debug!("Stored connection metrics for {}", conn_id);
        }
    }
    async fn inbounds(&self) {
        let node = &self.node;
        let now = Utc::now().timestamp();

        let node_id = self.node_id();

        for (tag, inbound) in node.inbounds.clone() {
            let base = format!("inbound.{}", tag);
            self.metrics.write(
                node_id,
                &format!("{base}.uplink"),
                inbound.uplink.unwrap_or(0) as f64,
                now,
            );
            self.metrics.write(
                node_id,
                &format!("{base}.downlink"),
                inbound.downlink.unwrap_or(0) as f64,
                now,
            );
            self.metrics.write(
                node_id,
                &format!("{base}.conn_count"),
                inbound.conn_count.unwrap_or(0) as f64,
                now,
            );
            log::debug!("Stored inbound metrics for {}", tag);
        }
    }
}
