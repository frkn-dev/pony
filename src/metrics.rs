use crate::fmt;
use serde::{Deserialize, Serialize};

use crate::config2::Settings;

pub trait AsMetric {
    type Output;

    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<Self::Output>>;
}

#[derive(Serialize, Deserialize)]
pub struct Metric<T> {
    pub path: String,
    pub value: T,
    pub timestamp: u64,
}

impl<T: Default> Default for Metric<T> {
    fn default() -> Self {
        Metric {
            value: T::default(),
            path: String::from(""),
            timestamp: 0,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Metric<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Metric {{ path: {}, value: {},  timestamp: {} }}",
            self.path, self.value, self.timestamp
        )
    }
}

impl<T: ToString> Metric<T> {
    pub fn to_string(&self) -> String {
        format!(
            "{} {} {}\nf",
            self.path,
            self.value.to_string(),
            self.timestamp
        )
    }
}

impl<T> Metric<T> {
    pub fn new(path: String, value: T, timestamp: u64) -> Self {
        Metric {
            path,
            value,
            timestamp,
        }
    }
}
