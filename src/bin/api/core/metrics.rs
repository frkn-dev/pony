use pony::metrics::storage::MetricStorage;
use pony::metrics::MetricEnvelope;
use pony::zmq::subscriber::Subscriber;

use rkyv::Deserialize;
use std::sync::Arc;

pub struct MetricWorker;

impl MetricWorker {
    pub async fn start(metric_storage: Arc<MetricStorage>, subscriber: Subscriber) {
        tokio::spawn(async move {
            log::info!("MetricWorker: Monitoring pipeline initialized...");

            loop {
                if let Some((_topic, payload_bytes)) = subscriber.recv().await {
                    let archived =
                        match rkyv::check_archived_root::<Vec<MetricEnvelope>>(&payload_bytes) {
                            Ok(a) => a,
                            Err(e) => {
                                log::error!("MetricWorker: Binary corruption detected: {:?}", e);
                                continue;
                            }
                        };

                    let metrics: Vec<MetricEnvelope> = match archived
                        .deserialize(&mut rkyv::Infallible)
                    {
                        Ok(m) => m,
                        Err(e) => {
                            log::error!("MetricWorker: Failed to reconstruct envelopes: {:?}", e);
                            continue;
                        }
                    };

                    for metric in metrics {
                        log::debug!(
                            "Incoming: node={} metric={} value={} tags={:?}",
                            metric.node_id,
                            metric.name,
                            metric.value,
                            metric.tags
                        );

                        metric_storage.insert_envelope(metric);
                    }
                }
            }
        });
    }
}
