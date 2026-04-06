use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

pub mod impls;
pub mod storage;

#[derive(Clone, Debug, SerdeDeserialize, SerdeSerialize)]
pub struct MetricPoint {
    pub timestamp: i64,
    pub value: f64,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct MetricEnvelope {
    pub node_id: uuid::Uuid,
    pub name: String,
    pub value: f64,
    pub timestamp: i64,
}
pub trait Metrics {
    fn heartbeat(&self) -> impl std::future::Future<Output = ()> + Send;
    fn memory(&self) -> impl std::future::Future<Output = ()> + Send;
    fn bandwidth(&self) -> impl std::future::Future<Output = ()> + Send;
    fn cpu_usage(&self) -> impl std::future::Future<Output = ()> + Send;
    fn loadavg(&self) -> impl std::future::Future<Output = ()> + Send;
}
