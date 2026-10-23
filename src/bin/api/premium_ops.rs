//! Premium detach, shared by the admin endpoint and the expiry worker.
//!
//! Order matters and is the whole point of this module: connections are what
//! actually keeps the VPN up, so they are torn down first, then the subscription
//! loses its premium anchor, and only then is the slot returned to stock. A slot
//! is never free while a hysteria token issued for it still authenticates.
//!
//! Deleting a connection goes through [`SyncOp::delete_connection`], which is
//! what updates the database, the in-memory cache and the ZMQ bus (nodes drop the
//! peer, auth drops the token). A plain SQL update would leave live peers behind.

use tracing::{error, warn};
use uuid::Uuid;

use fcore::{
    Connection, ConnectionApiOperations, ConnectionBaseOperations, ConnectionStorageApiOperations,
    Env, NodeStorageOperations, Result, Subscription, SubscriptionOperations,
    SubscriptionStorageOperations,
};

use crate::sync::{tasks::SyncOp, MemSync};

/// What a detach did, for the caller's response or the worker's log line.
#[derive(Debug, Clone)]
pub struct DetachSummary {
    /// Premium env the subscription was anchored to, when known.
    pub env: Option<Env>,
    /// Slot that went back to stock.
    pub slot_key: Option<String>,
    pub connections_deleted: usize,
    pub connections_failed: usize,
}

/// Detach premium from a subscription.
///
/// `Ok(None)` means there was nothing to detach: the subscription is unknown to
/// the cache *and* owns no slot (the worker sees plenty of those on every tick
/// once a subscription is gone).
pub async fn detach_premium<N, C, S>(
    memory: &MemSync<N, C, S>,
    subscription_id: Uuid,
    reason: &str,
) -> Result<Option<DetachSummary>>
where
    N: NodeStorageOperations + Sync + Send + Clone + 'static,
    C: ConnectionApiOperations
        + ConnectionBaseOperations
        + Sync
        + Send
        + Clone
        + 'static
        + From<Connection>
        + PartialEq,
    Connection: From<C>,
    S: SubscriptionOperations
        + Send
        + Sync
        + Clone
        + 'static
        + PartialEq
        + From<Subscription>,
{
    // Connections living in the premium env. Anything outside it belongs to the
    // base subscription and must survive the detach.
    let (env, doomed): (Option<Env>, Vec<(Uuid, C)>) = {
        let mem = memory.memory.read().await;
        match mem.subscriptions.find_by_id(&subscription_id) {
            Some(sub) => {
                let env = sub.scope_env().cloned();
                let conns = match &env {
                    Some(env) => mem
                        .connections
                        .get_by_subscription_id(&subscription_id)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|(_, conn)| conn.get_env() == *env && !conn.get_deleted())
                        .collect(),
                    None => Vec::new(),
                };
                (env, conns)
            }
            // A deleted subscription is gone from the cache, but its slot row
            // (if any) still has to be released.
            None => (None, Vec::new()),
        }
    };

    let mut connections_deleted = 0usize;
    let mut connections_failed = 0usize;

    for (conn_id, conn) in &doomed {
        match SyncOp::delete_connection(memory, conn_id, conn, Some(reason)).await {
            Ok(_) => connections_deleted += 1,
            Err(e) => {
                connections_failed += 1;
                error!("Failed to detach premium connection {}: {:?}", conn_id, e);
            }
        }
    }

    // Frees the slot and clears `scope_env` / `premium_token` / `premium_until`
    // in one transaction.
    let slot = memory.db.premium_slots().release(subscription_id).await?;

    {
        let mut mem = memory.memory.write().await;
        if let Some(sub) = mem.subscriptions.find_by_id_mut(&subscription_id) {
            sub.clear_premium();
        }
    }

    if slot.is_none() && env.is_none() && doomed.is_empty() {
        // Nothing was attached; keep the caller honest instead of reporting a
        // fake detach.
        return Ok(None);
    }

    if connections_failed > 0 {
        warn!(
            "Premium detach for {} left {} connections alive; they will be retried",
            subscription_id, connections_failed
        );
    }

    Ok(Some(DetachSummary {
        env,
        slot_key: slot.map(|slot| slot.slot_key),
        connections_deleted,
        connections_failed,
    }))
}
