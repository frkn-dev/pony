use serde::{Deserialize, Serialize};

use super::stats::InboundStat;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InboundResponse {
    port: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Inbound {
    port: u16,
    uplink: Option<i64>,
    downlink: Option<i64>,
    user_count: Option<i64>,
}

impl Inbound {
    pub fn new(port: u16) -> Self {
        Self {
            port: port,
            uplink: Some(0),
            downlink: Some(0),
            user_count: Some(0),
        }
    }

    pub fn as_inbound_response(&self) -> InboundResponse {
        InboundResponse { port: self.port }
    }

    pub fn as_inbound_stat(&self) -> InboundStat {
        InboundStat {
            uplink: self.uplink.unwrap_or(0),
            downlink: self.downlink.unwrap_or(0),
            user_count: self.user_count.unwrap_or(0),
        }
    }

    pub fn update_uplink(&mut self, new_uplink: i64) {
        self.uplink = Some(new_uplink);
    }

    pub fn update_downlink(&mut self, new_downlink: i64) {
        self.downlink = Some(new_downlink);
    }
    pub fn update_user_count(&mut self, new_user_count: i64) {
        self.user_count = Some(new_user_count);
    }
}
