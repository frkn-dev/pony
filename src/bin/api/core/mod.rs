use std::sync::Arc;
use tokio::sync::Mutex;

use pony::clickhouse::ChContext;
use pony::config::settings::ApiSettings;
use pony::postgres::PgContext;
use pony::state::connection::Conn;
use pony::state::connection::ConnApiOp;
use pony::state::connection::ConnBaseOp;
use pony::state::state::NodeStorage;
use pony::state::state::State;
use pony::zmq::publisher::Publisher as ZmqPublisher;

pub mod http;
pub mod tasks;

pub struct Api<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub state: Arc<Mutex<State<T, C>>>,
    pub db: PgContext,
    pub ch: ChContext,
    pub publisher: ZmqPublisher,
    pub settings: ApiSettings,
}

impl<T, C> Api<T, C>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub fn new(
        db: PgContext,
        ch: ChContext,
        publisher: ZmqPublisher,
        state: Arc<Mutex<State<T, C>>>,
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
