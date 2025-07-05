use chrono::NaiveDateTime;
use chrono::Utc;

use crate::state::connection::base::Base;
use crate::state::connection::conn::Conn;
use crate::state::connection::proto::Proto;
use crate::state::connection::stat::Stat;
use crate::state::connection::wireguard::Param as WgParam;
use crate::{PonyError, Result};

pub trait Operations {
    fn get_uplink(&self) -> i64;
    fn set_uplink(&mut self, v: i64);
    fn reset_uplink(&mut self);

    fn get_downlink(&self) -> i64;
    fn set_downlink(&mut self, v: i64);
    fn reset_downlink(&mut self);

    fn get_online(&self) -> i64;
    fn set_online(&mut self, v: i64);

    fn get_modified_at(&self) -> NaiveDateTime;
    fn set_modified_at(&mut self);

    fn set_proto(&mut self, proto: Proto);
    fn get_proto(&self) -> Proto;

    fn set_deleted(&mut self, v: bool);
    fn get_deleted(&self) -> bool;

    fn as_conn_stat(&self) -> Stat;

    fn get_wireguard(&self) -> Option<&WgParam>;
    fn get_wireguard_node_id(&self) -> Option<uuid::Uuid>;
    fn get_password(&self) -> Option<String>;
    fn set_password(&mut self, password: Option<String>) -> Result<()>;
}

impl Operations for Base {
    fn get_uplink(&self) -> i64 {
        self.stat.uplink
    }
    fn set_uplink(&mut self, v: i64) {
        self.stat.uplink = v;
    }
    fn reset_uplink(&mut self) {
        self.stat.uplink = 0;
    }

    fn get_downlink(&self) -> i64 {
        self.stat.downlink
    }
    fn set_downlink(&mut self, v: i64) {
        self.stat.downlink = v;
    }
    fn reset_downlink(&mut self) {
        self.stat.downlink = 0;
    }

    fn get_online(&self) -> i64 {
        self.stat.online
    }
    fn set_online(&mut self, v: i64) {
        self.stat.online = v;
    }

    fn get_modified_at(&self) -> NaiveDateTime {
        self.modified_at
    }
    fn set_modified_at(&mut self) {
        self.modified_at = Utc::now().naive_utc();
    }

    fn set_proto(&mut self, proto: Proto) {
        self.proto = proto
    }
    fn get_proto(&self) -> Proto {
        self.proto.clone()
    }

    fn as_conn_stat(&self) -> Stat {
        Stat {
            uplink: self.stat.uplink,
            downlink: self.stat.downlink,
            online: self.stat.online,
        }
    }

    fn set_deleted(&mut self, v: bool) {
        self.is_deleted = v;
    }
    fn get_deleted(&self) -> bool {
        self.is_deleted
    }

    fn get_wireguard_node_id(&self) -> Option<uuid::Uuid> {
        match &self.proto {
            Proto::Wireguard { node_id, .. } => Some(*node_id),
            _ => None,
        }
    }

    fn get_password(&self) -> Option<String> {
        match &self.proto {
            Proto::Shadowsocks { password } => Some(password.clone()),
            _ => None,
        }
    }

    fn get_wireguard(&self) -> Option<&WgParam> {
        match &self.proto {
            Proto::Wireguard { param, .. } => Some(param),
            _ => None,
        }
    }

    fn set_password(&mut self, password: Option<String>) -> Result<()> {
        match (&mut self.proto, password) {
            (Proto::Shadowsocks { password: p }, Some(new_pw)) => {
                *p = new_pw;
                Ok(())
            }
            _ => Err(PonyError::Custom(
                "Password update failed: not a Shadowsocks connection".into(),
            )),
        }
    }
}

impl Operations for Conn {
    fn get_uplink(&self) -> i64 {
        self.stat.uplink
    }
    fn set_uplink(&mut self, v: i64) {
        self.stat.uplink = v;
    }
    fn reset_uplink(&mut self) {
        self.stat.uplink = 0;
    }

    fn get_downlink(&self) -> i64 {
        self.stat.downlink
    }
    fn set_downlink(&mut self, v: i64) {
        self.stat.downlink = v;
    }
    fn reset_downlink(&mut self) {
        self.stat.downlink = 0;
    }

    fn get_online(&self) -> i64 {
        self.stat.online
    }
    fn set_online(&mut self, v: i64) {
        self.stat.online = v;
    }

    fn get_modified_at(&self) -> NaiveDateTime {
        self.modified_at
    }
    fn set_modified_at(&mut self) {
        self.modified_at = Utc::now().naive_utc();
    }

    fn set_proto(&mut self, proto: Proto) {
        self.proto = proto
    }
    fn get_proto(&self) -> Proto {
        self.proto.clone()
    }

    fn set_deleted(&mut self, v: bool) {
        self.is_deleted = v;
    }
    fn get_deleted(&self) -> bool {
        self.is_deleted
    }

    fn as_conn_stat(&self) -> Stat {
        Stat {
            uplink: self.stat.uplink,
            downlink: self.stat.downlink,
            online: self.stat.online,
        }
    }

    fn get_wireguard_node_id(&self) -> Option<uuid::Uuid> {
        match &self.proto {
            Proto::Wireguard { node_id, .. } => Some(*node_id),
            _ => None,
        }
    }

    fn get_password(&self) -> Option<String> {
        match &self.proto {
            Proto::Shadowsocks { password } => Some(password.clone()),
            _ => None,
        }
    }

    fn get_wireguard(&self) -> Option<&WgParam> {
        match &self.proto {
            Proto::Wireguard { param, .. } => Some(param),
            _ => None,
        }
    }

    fn set_password(&mut self, password: Option<String>) -> Result<()> {
        match (&mut self.proto, password) {
            (Proto::Shadowsocks { password: p }, Some(new_pw)) => {
                *p = new_pw;
                Ok(())
            }
            _ => Err(PonyError::Custom(
                "Password update failed: not a Shadowsocks connection".into(),
            )),
        }
    }
}
