use pony::clickhouse::ChContext;
use pony::config::settings::ApiSettings;
use pony::state::ConnApiOp;
use pony::state::ConnBaseOp;
use pony::state::NodeStorage;
use pony::state::SyncState;
use pony::zmq::publisher::Publisher as ZmqPublisher;

pub mod http;
pub(crate) mod metrics;
pub mod tasks;

pub struct Api<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub state: SyncState<N, C>,
    pub ch: ChContext,
    pub publisher: ZmqPublisher,
    pub settings: ApiSettings,
}

impl<N, C> Api<N, C>
where
    N: NodeStorage + Send + Sync + Clone + 'static,
    C: ConnApiOp + ConnBaseOp + Send + Sync + Clone + 'static,
{
    pub fn new(
        ch: ChContext,
        publisher: ZmqPublisher,
        state: SyncState<N, C>,
        settings: ApiSettings,
    ) -> Self {
        Self {
            ch,
            publisher,
            state,
            settings,
        }
    }
}
