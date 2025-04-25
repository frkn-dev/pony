use std::sync::Arc;

use tokio::sync::Mutex;

use crate::ChContext;
use crate::DbContext;
use crate::NodeStorage;
use crate::State;
use crate::ZmqPublisher;

pub mod tasks;

pub struct Api<T>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    pub db: DbContext,
    pub ch: ChContext,
    pub publisher: ZmqPublisher,
    pub state: Arc<Mutex<State<T>>>,
}

impl<T> Api<T>
where
    T: NodeStorage + Send + Sync + Clone + 'static,
{
    pub fn new(
        db: DbContext,
        ch: ChContext,
        publisher: ZmqPublisher,
        state: Arc<Mutex<State<T>>>,
    ) -> Self {
        Self {
            db,
            ch,
            publisher,
            state,
        }
    }
}
