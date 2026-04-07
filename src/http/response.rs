use crate::memory::connection::conn::Conn as Connection;
use crate::memory::connection::stat::Stat as ConnectionStat;
use crate::memory::key::Key;
use crate::memory::subscription::Subscription;
use crate::memory::tag::ProtoTag as Tag;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ResponseMessage<T> {
    pub status: u16,
    pub message: String,
    pub response: T,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceWithId<T> {
    pub id: uuid::Uuid,
    pub instance: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Instance {
    Connection(Connection),
    Subscription(Subscription),
    Stat(Vec<(uuid::Uuid, ConnectionStat, Tag)>),
    Connections(Vec<(uuid::Uuid, Connection)>),
    Key(Key),
    Count(usize),
    None,
}
