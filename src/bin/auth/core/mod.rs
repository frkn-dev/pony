use pony::Subscription;
use std::sync::Arc;
use tokio::sync::RwLock;

use pony::memory::node::Node;
use pony::zmq::subscriber::Subscriber as ZmqSubscriber;
use pony::BaseConnection as Connection;
use pony::ConnectionBaseOp;
use pony::MemoryCache;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

pub mod http;
pub mod service;
pub mod tasks;

pub type AuthServiceState = MemoryCache<Node, Connection, Subscription>;

pub struct AuthService<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub memory: Arc<RwLock<MemoryCache<N, C, S>>>,
    pub subscriber: ZmqSubscriber,
}

impl<N, C, S> AuthService<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + Send + Sync + Clone + 'static,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub fn new(memory: Arc<RwLock<MemoryCache<N, C, S>>>, subscriber: ZmqSubscriber) -> Self {
        Self { memory, subscriber }
    }
}
