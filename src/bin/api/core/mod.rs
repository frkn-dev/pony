use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use pony::config::settings::ApiSettings;
use pony::memory::connection::Connections;
use pony::memory::node::Node;
use pony::memory::subscription::Subscription;
use pony::memory::subscription::Subscriptions;
use pony::Connection;
use pony::ConnectionApiOp;
use pony::ConnectionBaseOp;
use pony::NodeStorageOp;
use pony::SubscriptionOp;

use crate::core::clickhouse::ChContext;
use crate::core::sync::MemSync;

pub(crate) mod clickhouse;
pub(crate) mod http;
pub(crate) mod metrics;
pub(crate) mod postgres;
pub(crate) mod sync;
pub(crate) mod tasks;

pub type ApiState = Cache<HashMap<String, Vec<Node>>, Connection, Subscription>;

pub struct Api<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub sync: MemSync<N, C, S>,
    pub ch: ChContext,
    pub settings: ApiSettings,
}

impl<N, C, S> Api<N, C, S>
where
    N: NodeStorageOp + Send + Sync + Clone + 'static,
    C: ConnectionBaseOp + ConnectionApiOp + Send + Sync + Clone + 'static + std::cmp::PartialEq,
    S: SubscriptionOp + Send + Sync + Clone + 'static,
{
    pub fn new(ch: ChContext, sync: MemSync<N, C, S>, settings: ApiSettings) -> Self {
        Self { ch, sync, settings }
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

impl<T: Default, C, S: Default + std::cmp::PartialEq> Default for Cache<T, C, S>
where
    T: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp + ConnectionBaseOp + Clone + Send + Sync + 'static + PartialEq,
    S: SubscriptionOp + Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default, C, S: Default + std::cmp::PartialEq> Cache<T, C, S>
where
    T: NodeStorageOp + Sync + Send + Clone + 'static,
    C: ConnectionApiOp + ConnectionBaseOp + Clone + Send + Sync + 'static + PartialEq,
    S: SubscriptionOp + Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Cache {
            nodes: T::default(),
            connections: Connections::default(),
            subscriptions: Subscriptions::default(),
        }
    }
}
