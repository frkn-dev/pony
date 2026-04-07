use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(
    Archive,
    Default,
    Deserialize,
    Serialize,
    RkyvDeserialize,
    RkyvSerialize,
    Debug,
    Clone,
    PartialEq,
)]
pub struct Stat {
    pub downlink: i64,
    pub uplink: i64,
    pub online: i64,
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Uplink: {:.2} MB, Downlink: {:.2} MB, Online: {} ",
            self.uplink as f64 / 1_048_576.0,
            self.downlink as f64 / 1_048_576.0,
            self.online
        )
    }
}
