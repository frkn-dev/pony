use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use pony::{
    Connection, ConnectionApiOperations, ConnectionBaseOperations, Connections, Env, MetricStorage,
    Node, NodeStorageOperations, Subscription, SubscriptionOperations, Subscriptions,
};

use super::config::ApiSettings;
use super::sync::MemSync;

pub type ApiState = Cache<HashMap<Env, Vec<Node>>, Connection, Subscription>;

pub struct Api<N, C, S>
where
    N: NodeStorageOperations + Send + Sync + Clone + 'static,
    C: ConnectionBaseOperations
        + ConnectionApiOperations
        + Send
        + Sync
        + Clone
        + 'static
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static,
{
    pub sync: MemSync<N, C, S>,
    pub settings: ApiSettings,
    pub metrics: Arc<MetricStorage>,
}

impl<N, C, S> Api<N, C, S>
where
    N: NodeStorageOperations + Send + Sync + Clone + 'static,
    C: ConnectionBaseOperations
        + ConnectionApiOperations
        + Send
        + Sync
        + Clone
        + 'static
        + PartialEq,
    S: SubscriptionOperations + Send + Sync + Clone + 'static,
{
    pub fn new(sync: MemSync<N, C, S>, settings: ApiSettings, metrics: Arc<MetricStorage>) -> Self {
        Self {
            sync,
            settings,
            metrics,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Cache<T, C, S>
where
    T: Send + Sync + Clone + 'static,
    C: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    pub connections: Connections<C>,
    pub subscriptions: Subscriptions<S>,
    pub nodes: T,
}

impl<T: Default, C, S: Default + PartialEq> Default for Cache<T, C, S>
where
    T: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Clone
        + Send
        + Sync
        + 'static
        + PartialEq,
    S: SubscriptionOperations + Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default, C, S: Default + PartialEq> Cache<T, C, S>
where
    T: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Clone
        + Send
        + Sync
        + 'static
        + PartialEq,
    S: SubscriptionOperations + Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Cache {
            nodes: T::default(),
            connections: Connections::default(),
            subscriptions: Subscriptions::default(),
        }
    }
}
