use std::fmt;

#[derive(Debug, Clone)]
pub enum Stat {
    Conn(Kind),
    Inbound(Kind),
    Outbound(Kind),
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stat::Conn(Kind::Uplink) => write!(f, "uplink"),
            Stat::Conn(Kind::Downlink) => write!(f, "downlink"),
            Stat::Conn(Kind::Online) => write!(f, "online"),
            Stat::Inbound(Kind::Uplink) => write!(f, "uplink"),
            Stat::Inbound(Kind::Downlink) => write!(f, "downlink"),
            Stat::Inbound(Kind::Online) => write!(f, "Not implemented"),
            Stat::Outbound(Kind::Uplink) => write!(f, "uplink"),
            Stat::Outbound(Kind::Downlink) => write!(f, "downlink"),
            Stat::Outbound(Kind::Online) => write!(f, "Not implemented"),

            Stat::Conn(Kind::Unknown)
            | Stat::Inbound(Kind::Unknown)
            | Stat::Outbound(Kind::Unknown) => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Kind {
    Uplink,
    Downlink,
    Online,
    Unknown,
}

impl Kind {
    pub fn from_path(path: &str) -> Kind {
        if let Some(last) = path.split('.').last() {
            match last {
                "uplink" => Kind::Uplink,
                "downlink" => Kind::Downlink,
                "online" => Kind::Online,
                _ => Kind::Unknown,
            }
        } else {
            Kind::Unknown
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Uplink => write!(f, "uplink"),
            Kind::Downlink => write!(f, "downlink"),
            Kind::Online => write!(f, "online"),
            Kind::Unknown => write!(f, "unknown"),
        }
    }
}
