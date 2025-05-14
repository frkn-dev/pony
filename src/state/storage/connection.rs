use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

use crate::state::connection::ConnApiOp;
use crate::state::connection::ConnBaseOp;
use crate::state::connection::ConnStatus;
use crate::state::state::Connections;
use crate::state::stats::StatType;
use crate::state::ConnStat;

use crate::{PonyError, Result};

pub trait ConnStorageApi<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn add_or_update(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<ConnStorageOpStatus>;
    fn restore(&mut self, conn_id: &uuid::Uuid) -> Result<()>;
    fn expire(&mut self, conn_id: &uuid::Uuid) -> Result<()>;
    fn update_limit(&mut self, conn_id: &uuid::Uuid, new_limit: i32) -> Result<()>;
    fn update_trial(&mut self, conn_id: &uuid::Uuid, new_trial: bool) -> Result<()>;
    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType);
    fn all_trial(&self, status: ConnStatus) -> HashMap<uuid::Uuid, C>;
    fn get_by_user_id(&self, user_id: &uuid::Uuid) -> Option<Vec<(uuid::Uuid, C)>>;
}

pub trait ConnStorageBase<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize;
    fn add(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<ConnStorageOpStatus>;
    fn remove(&mut self, conn_id: &uuid::Uuid) -> Result<()>;
    fn get(&self, conn_id: &uuid::Uuid) -> Option<C>;
    fn update_stat(&mut self, conn_id: &uuid::Uuid, stat: ConnStat) -> Result<()>;
    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType);
    fn update_uplink(&mut self, conn_id: &uuid::Uuid, new_uplink: i64) -> Result<()>;
    fn update_downlink(&mut self, conn_id: &uuid::Uuid, new_downlink: i64) -> Result<()>;
    fn update_online(&mut self, conn_id: &uuid::Uuid, new_online: i64) -> Result<()>;
    fn update_stats(&mut self, conn_id: &uuid::Uuid, stats: ConnStat) -> Result<()>;
}

#[derive(Debug)]
pub enum ConnStorageOpStatus {
    AlreadyExist,
    Updated,
    UpdatedStat,
    Ok,
}

impl fmt::Display for ConnStorageOpStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::AlreadyExist => write!(f, "AlreadyExist"),
            Self::Updated => write!(f, "Updated"),
            Self::Ok => write!(f, "Ok"),
            Self::UpdatedStat => write!(f, "UpdatedStat"),
        }
    }
}

impl<C> ConnStorageBase<C> for Connections<C>
where
    C: ConnBaseOp + Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        self.0.len()
    }

    fn add(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<ConnStorageOpStatus> {
        match self.entry(*conn_id) {
            Entry::Occupied(_) => return Ok(ConnStorageOpStatus::AlreadyExist),
            Entry::Vacant(entry) => {
                entry.insert(new_conn);
            }
        }
        Ok(ConnStorageOpStatus::Ok)
    }

    fn remove(&mut self, conn_id: &uuid::Uuid) -> Result<()> {
        self.0
            .remove(conn_id)
            .map(|_| ())
            .ok_or(PonyError::Custom("Conn not found".into()))
    }

    fn get(&self, conn_id: &uuid::Uuid) -> Option<C> {
        self.0.get(conn_id).cloned()
    }

    fn update_stats(&mut self, conn_id: &uuid::Uuid, stats: ConnStat) -> Result<()> {
        let conn = self
            .get_mut(conn_id)
            .ok_or(PonyError::Custom("Conn not found".into()))?;

        conn.set_online(stats.online);
        conn.set_uplink(stats.uplink);
        conn.set_downlink(stats.downlink);

        Ok(())
    }

    fn update_stat(&mut self, conn_id: &uuid::Uuid, stat: ConnStat) -> Result<()> {
        let conn = self
            .get_mut(&conn_id)
            .ok_or(PonyError::Custom("Conn not found".into()))?;
        conn.set_uplink(stat.uplink);
        conn.set_downlink(stat.downlink);
        conn.set_online(stat.online);
        conn.set_modified_at();
        Ok(())
    }

    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType) {
        if let Some(conn) = self.get_mut(conn_id) {
            match stat {
                StatType::Uplink => conn.reset_uplink(),
                StatType::Downlink => conn.reset_downlink(),
                StatType::Online => {}
                StatType::Unknown => {}
            }
        }
    }

    fn update_uplink(&mut self, conn_id: &uuid::Uuid, new_uplink: i64) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_uplink(new_uplink);
            conn.set_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom(
                format!("Conn not found: {}", conn_id).into(),
            ))
        }
    }

    fn update_downlink(&mut self, conn_id: &uuid::Uuid, new_downlink: i64) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_downlink(new_downlink);
            conn.set_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom(
                format!("Conn not found: {}", conn_id).into(),
            ))
        }
    }

    fn update_online(&mut self, conn_id: &uuid::Uuid, new_online: i64) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_online(new_online);
            conn.set_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom(
                format!("Conn not found: {}", conn_id).into(),
            ))
        }
    }
}

impl<C> ConnStorageApi<C> for Connections<C>
where
    C: ConnBaseOp + ConnApiOp + Clone + Send + Sync + 'static,
{
    fn add_or_update(&mut self, conn_id: &uuid::Uuid, new_conn: C) -> Result<ConnStorageOpStatus> {
        match self.entry(*conn_id) {
            Entry::Occupied(mut entry) => {
                let existing_conn = entry.get_mut();

                if let Some(user_id) = new_conn.get_user_id() {
                    existing_conn.set_user_id(&user_id);
                }

                existing_conn.set_trial(new_conn.get_trial());
                existing_conn.set_limit(new_conn.get_limit());
                existing_conn.set_password(new_conn.get_password());
                existing_conn.set_status(new_conn.get_status());

                Ok(ConnStorageOpStatus::Updated)
            }
            Entry::Vacant(entry) => {
                entry.insert(new_conn);
                Ok(ConnStorageOpStatus::Ok)
            }
        }
    }

    fn get_by_user_id(&self, user_id: &uuid::Uuid) -> Option<Vec<(uuid::Uuid, C)>> {
        let conns: Vec<(uuid::Uuid, C)> = self
            .iter()
            .filter(|(_, conn)| conn.get_user_id() == Some(*user_id))
            .map(|(conn_id, conn)| (*conn_id, conn.clone()))
            .collect();

        if conns.is_empty() {
            None
        } else {
            Some(conns)
        }
    }

    fn restore(&mut self, conn_id: &uuid::Uuid) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_status(ConnStatus::Active);
            conn.set_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom("Conn not found".into()))
        }
    }

    fn expire(&mut self, conn_id: &uuid::Uuid) -> Result<()> {
        if let Some(conn) = self.get_mut(&conn_id) {
            conn.set_status(ConnStatus::Expired);
            conn.set_modified_at();
            Ok(())
        } else {
            Err(PonyError::Custom("Conn not found".into()))
        }
    }

    fn update_limit(&mut self, conn_id: &uuid::Uuid, new_limit: i32) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_limit(new_limit);
            conn.set_modified_at();
        }
        Ok(())
    }

    fn update_trial(&mut self, conn_id: &uuid::Uuid, new_trial: bool) -> Result<()> {
        if let Some(conn) = self.get_mut(conn_id) {
            conn.set_trial(new_trial);
            conn.set_modified_at();
        }
        Ok(())
    }

    fn reset_stat(&mut self, conn_id: &uuid::Uuid, stat: StatType) {
        if let Some(conn) = self.get_mut(conn_id) {
            match stat {
                StatType::Uplink => conn.reset_uplink(),
                StatType::Downlink => conn.reset_downlink(),
                StatType::Online => {}
                StatType::Unknown => {}
            }
        }
    }

    fn all_trial(&self, status: ConnStatus) -> HashMap<uuid::Uuid, C> {
        self.iter()
            .filter(|(_, conn)| conn.get_status() == status && conn.get_trial())
            .map(|(conn_id, conn)| (*conn_id, conn.clone()))
            .collect()
    }
}
