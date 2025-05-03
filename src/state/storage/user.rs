use crate::Result;
use std::collections::HashMap;

use crate::state::user::User;

pub trait UserStorage {
    fn try_add(&mut self, user_id: uuid::Uuid, user: User) -> Result<UserStorageOpStatus>;
    fn by_username(&self, username: &str) -> Option<uuid::Uuid>;
}

pub enum UserStorageOpStatus {
    AlreadyExist,
    Ok,
}

impl UserStorage for HashMap<uuid::Uuid, User> {
    fn by_username(&self, username: &str) -> Option<uuid::Uuid> {
        self.iter()
            .find(|(_, u)| u.username == username)
            .map(|(id, _)| *id)
    }

    fn try_add(&mut self, user_id: uuid::Uuid, user: User) -> Result<UserStorageOpStatus> {
        if self.values().any(|u| u.username == user.username) {
            return Ok(UserStorageOpStatus::AlreadyExist);
        }
        self.insert(user_id, user);
        Ok(UserStorageOpStatus::Ok)
    }
}
