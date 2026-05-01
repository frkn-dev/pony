use crate::memory::{env::Env, tag::ProtoTag as Tag};
use crate::zmq::topic::Topic;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnType {
    pub proto: Tag,
    pub last_update: Option<u64>,
    pub env: Env,
    pub topic: Topic,
}
