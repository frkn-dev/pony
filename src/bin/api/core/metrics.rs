use pony::metrics::storage::HasMetrics;
use pony::metrics::storage::MetricSink;
use pony::metrics::storage::MetricStorage;
use pony::metrics::MetricEnvelope;
use pony::zmq::subscriber::Subscriber;

use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

use rkyv::Deserialize;
use std::sync::Arc;

use super::Api;

impl<N, C, S> HasMetrics for Api<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    fn metrics(&self) -> &MetricStorage {
        &self.metrics
    }

    fn node_id(&self) -> &uuid::Uuid {
        &self.settings.node.uuid
    }
}

pub struct MetricWorker;

impl MetricWorker {
    pub async fn start<N, C, S>(
        metric_storage: Arc<MetricStorage>,
        subscriber: Subscriber,
        // Канал для ClickHouse/Carbon
        carbon_tx: tokio::sync::mpsc::UnboundedSender<MetricEnvelope>,
    ) where
        N: Send + Sync + Clone + 'static,
        C: Send + Sync + Clone + 'static,
        S: Send + Sync + Clone + 'static,
    {
        tokio::spawn(async move {
            log::info!("MetricWorker: Started listening for metrics...");

            loop {
                // Используем твой существующий recv()
                if let Some((_topic_bytes, payload_bytes)) = subscriber.recv().await {
                    // Десериализуем rkyv (быстро и безопасно)
                    let archived = match rkyv::check_archived_root::<MetricEnvelope>(&payload_bytes)
                    {
                        Ok(a) => a,
                        Err(e) => {
                            log::error!("MetricWorker: Invalid rkyv root: {:?}", e);
                            continue;
                        }
                    };

                    let msg: MetricEnvelope = match archived.deserialize(&mut rkyv::Infallible) {
                        Ok(m) => m,
                        Err(e) => {
                            log::error!("MetricWorker: Deserialize failed: {:?}", e);
                            continue;
                        }
                    };

                    // 1. Сохраняем в локальный сторадж API (Cache)
                    // Ключ: "node_id.metric_name"
                    let storage_key = format!("{}.{}", msg.node_id, msg.name);
                    log::debug!("{} | {} | {}", storage_key, msg.value, msg.timestamp);
                    metric_storage.write(&msg.node_id, &storage_key, msg.value, msg.timestamp);

                    //let _ = carbon_tx.send(msg);
                }
            }
        });
    }
}
