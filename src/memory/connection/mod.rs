use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

pub(crate) mod base;
pub(crate) mod conn;
pub(crate) mod operation;
pub(crate) mod proto;
pub mod stat;
pub mod wireguard;

#[derive(Archive, Deserialize, Serialize, RkyvDeserialize, RkyvSerialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Connections<C>(pub HashMap<uuid::Uuid, C>);

impl<C> Default for Connections<C> {
    fn default() -> Self {
        Connections(HashMap::new())
    }
}

impl<C: fmt::Display> fmt::Display for Connections<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (uuid, conn) in &self.0 {
            writeln!(f, "{} => {}", uuid, conn)?;
        }
        Ok(())
    }
}

impl<C> Deref for Connections<C> {
    type Target = HashMap<uuid::Uuid, C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C> DerefMut for Connections<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
