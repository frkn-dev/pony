use chrono::Utc;
use rkyv::with::AsOwned;
use rkyv::with::With;
use rkyv::Infallible;
use rkyv::{to_bytes, Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
use std::path::Path;
use std::sync::Arc;
use tokio::fs as async_fs;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::MemoryCache;

use super::cache::Connections;
use super::connection::conn::Conn;

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct SnapshotData<C>
where
    C: Archive + Send + Sync + Clone + 'static,
{
    pub timestamp: u64,
    pub memory: Connections<C>,
    pub version: u32,
}

pub struct SnapshotManager<N, C, S>
where
    C: Archive + Send + Sync + Clone + 'static,
    N: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    pub snapshot_path: String,
    pub memory: Arc<RwLock<MemoryCache<N, C, S>>>,
}

impl<N, C, S> Clone for SnapshotManager<N, C, S>
where
    C: Archive
        + Send
        + Sync
        + Clone
        + 'static
        + std::convert::From<Conn>
        + rkyv::Serialize<
            rkyv::ser::serializers::CompositeSerializer<
                rkyv::ser::serializers::AlignedSerializer<rkyv::AlignedVec>,
                rkyv::ser::serializers::FallbackScratch<
                    rkyv::ser::serializers::HeapScratch<256>,
                    rkyv::ser::serializers::AllocScratch,
                >,
                rkyv::ser::serializers::SharedSerializeMap,
            >,
        >,
    N: Send + Sync + Clone,
    S: Send + Sync + Clone + 'static,
{
    fn clone(&self) -> Self {
        SnapshotManager {
            snapshot_path: self.snapshot_path.clone(),
            memory: self.memory.clone(),
        }
    }
}

impl<N, C, S> SnapshotManager<N, C, S>
where
    C: Archive
        + Send
        + Sync
        + Clone
        + 'static
        + std::convert::From<Conn>
        + rkyv::Serialize<
            rkyv::ser::serializers::CompositeSerializer<
                rkyv::ser::serializers::AlignedSerializer<rkyv::AlignedVec>,
                rkyv::ser::serializers::FallbackScratch<
                    rkyv::ser::serializers::HeapScratch<256>,
                    rkyv::ser::serializers::AllocScratch,
                >,
                rkyv::ser::serializers::SharedSerializeMap,
            >,
        >,
    N: Send + Sync + Clone + 'static,
    S: Send + Sync + Clone + 'static,
{
    pub fn new(snapshot_path: String, memory: Arc<RwLock<MemoryCache<N, C, S>>>) -> Self {
        Self {
            snapshot_path,
            memory,
        }
    }

    pub async fn create_snapshot(&self) -> Result<()> {
        let memory_guard = self.memory.read().await;
        let connections = memory_guard.connections.clone();
        let connections_count = connections.len();
        drop(memory_guard);

        let snapshot = SnapshotData {
            timestamp: Utc::now().timestamp() as u64,
            memory: connections,
            version: 1,
        };

        let bytes = to_bytes::<_, 256>(&snapshot)?;

        let temp_path = format!("{}.tmp", self.snapshot_path);
        async_fs::write(&temp_path, &bytes).await?;
        async_fs::rename(&temp_path, &self.snapshot_path).await?;

        log::debug!(
            "Snapshot created at: {} ({} bytes, {} connections)",
            snapshot.timestamp,
            bytes.len(),
            connections_count
        );
        Ok(())
    }

    pub async fn load_snapshot(&self) -> Result<Option<u64>>
    where
        <C as Archive>::Archived: rkyv::Deserialize<C, rkyv::Infallible>,
    {
        if !Path::new(&self.snapshot_path).exists() {
            return Ok(None);
        }

        let bytes = async_fs::read(&self.snapshot_path).await?;
        let archived = unsafe { rkyv::archived_root::<SnapshotData<C>>(&bytes) };
        let with: With<SnapshotData<C>, AsOwned> = archived.deserialize(&mut Infallible)?;
        let snapshot: SnapshotData<C> = with.into_inner();

        let mut memory_guard = self.memory.write().await;
        memory_guard.connections = snapshot.memory.clone();

        log::debug!(
            "Snapshot loaded from: {} bytes {}",
            snapshot.timestamp,
            bytes.len()
        );
        Ok(Some(snapshot.timestamp))
    }

    pub async fn get_snapshot_timestamp(&self) -> Result<Option<u64>> {
        if !Path::new(&self.snapshot_path).exists() {
            return Ok(None);
        }

        let bytes = async_fs::read(&self.snapshot_path).await?;
        let archived = unsafe { rkyv::archived_root::<SnapshotData<C>>(&bytes) };

        Ok(Some(archived.timestamp))
    }

    pub async fn count(&self) -> usize {
        let mem = self.memory.read().await;
        mem.connections.0.len()
    }
}

impl<C: Send + Sync + Clone + From<Conn>> Connections<C> {
    pub fn load_from_cache(&mut self, connections: Connections<C>) -> Result<()> {
        *self = connections;
        Ok(())
    }
}
