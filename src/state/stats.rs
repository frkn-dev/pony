use std::fmt;

#[derive(Debug, Clone)]
pub enum Stat {
    Conn(StatType),
    Inbound(StatType),
    Outbound(StatType),
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stat::Conn(StatType::Uplink) => write!(f, "uplink"),
            Stat::Conn(StatType::Downlink) => write!(f, "downlink"),
            Stat::Conn(StatType::Online) => write!(f, "online"),
            Stat::Inbound(StatType::Uplink) => write!(f, "uplink"),
            Stat::Inbound(StatType::Downlink) => write!(f, "downlink"),
            Stat::Inbound(StatType::Online) => write!(f, "Not implemented"),
            Stat::Outbound(StatType::Uplink) => write!(f, "uplink"),
            Stat::Outbound(StatType::Downlink) => write!(f, "downlink"),
            Stat::Outbound(StatType::Online) => write!(f, "Not implemented"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatType {
    Uplink,
    Downlink,
    Online,
}

impl fmt::Display for StatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatType::Uplink => write!(f, "uplink"),
            StatType::Downlink => write!(f, "downlink"),
            StatType::Online => write!(f, "online"),
        }
    }
}

pub struct ConnStat {
    pub downlink: i64,
    pub uplink: i64,
    pub online: i64,
}

pub struct InboundStat {
    pub downlink: i64,
    pub uplink: i64,
    pub conn_count: i64,
}
