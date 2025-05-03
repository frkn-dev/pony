use std::collections::HashMap;

pub(crate) mod connection;
mod node;
mod postgres;
mod state;
mod stats;
pub(crate) mod storage;
pub(crate) mod sync;
mod tag;
mod user;

pub type ApiState = State<HashMap<String, Vec<Node>>, Conn>;
pub type AgentState = State<Node, ConnBase>;

pub use connection::Conn;
pub use connection::ConnApiOp;
pub use connection::ConnBase;
pub use connection::ConnBaseOp;
pub use connection::ConnStatus;

pub use stats::StatType;
pub use sync::SyncOp;

pub use user::User;

pub use node::Node;
pub use node::NodeStatus;

pub use postgres::connection::ConnRow;
pub use postgres::user::UserRow;
pub use postgres::PgContext;

pub use sync::SyncState;
pub use sync::SyncTask;

pub use tag::Tag;

pub use state::Connections;
pub use state::State;

pub use stats::ConnStat;
pub use stats::InboundStat;
pub use stats::Stat;

pub use storage::connection::ConnStorageApi;
pub use storage::connection::ConnStorageBase;
pub use storage::node::NodeStorage;
pub use storage::node::NodeStorageOpStatus;
pub use storage::user::UserStorage;
pub use storage::user::UserStorageOpStatus;

pub use postgres::run_shadow_sync as pg_run_shadow_sync;
