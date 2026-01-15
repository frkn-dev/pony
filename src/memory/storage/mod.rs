use std::fmt;

pub mod connection;
pub(crate) mod node;
pub(crate) mod subscription;

#[derive(Debug, PartialEq)]
pub enum Status {
    AlreadyExist(uuid::Uuid),
    NotModified(uuid::Uuid),
    Updated(uuid::Uuid),
    UpdatedStat(uuid::Uuid),
    NotFound(uuid::Uuid),
    BadRequest(uuid::Uuid, String),
    Ok(uuid::Uuid),
    DeletedPreviously(uuid::Uuid),
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::AlreadyExist(uuid) => write!(f, "AlreadyExist: {uuid}"),
            Self::NotModified(uuid) => write!(f, "NotModified: {uuid}"),
            Self::Updated(uuid) => write!(f, "Updated: {uuid}"),
            Self::Ok(uuid) => write!(f, "Ok: {uuid}"),
            Self::NotFound(uuid) => write!(f, "NotFound: {uuid}"),
            Self::UpdatedStat(uuid) => write!(f, "UpdatedStat: {uuid}"),
            Self::BadRequest(uuid, msg) => write!(f, "BadRequest: {uuid} {msg}"),
            Self::DeletedPreviously(uuid) => write!(f, "DeletedPreviously: {uuid}"),
        }
    }
}
