use std::sync::Arc;
use tokio::sync::Mutex;

use crate::clickhouse::ChContext;
use crate::config::settings::ApiSettings;
use crate::postgres::PgContext;
use crate::state::state::NodeStorage;
use crate::state::state::State;
use crate::zmq::publisher::Publisher as ZmqPublisher;

pub mod http;
pub mod requests;
pub mod tasks;

pub struct Api<T>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    pub db: PgContext,
    pub ch: ChContext,
    pub publisher: ZmqPublisher,
    pub state: Arc<Mutex<State<T>>>,
    pub settings: ApiSettings,
}

impl<T> Api<T>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    pub fn new(
        db: PgContext,
        ch: ChContext,
        publisher: ZmqPublisher,
        state: Arc<Mutex<State<T>>>,
        settings: ApiSettings,
    ) -> Self {
        Self {
            db,
            ch,
            publisher,
            state,
            settings,
        }
    }
}
