use crate::Result;
use std::collections::HashMap;

use crate::state::user::User;

pub trait UserStorage {
    fn try_add(&mut self, user_id: &uuid::Uuid, user: User) -> Result<UserStorageOpStatus>;
    fn delete(&mut self, user_id: &uuid::Uuid) -> Result<()>;
    fn by_username(&self, username: &str) -> Option<uuid::Uuid>;
    fn all(&self) -> Result<Vec<(uuid::Uuid, User)>>;
}

pub enum UserStorageOpStatus {
    AlreadyExist,
    Updated,
    Ok,
}

impl UserStorage for HashMap<uuid::Uuid, User> {
    fn by_username(&self, username: &str) -> Option<uuid::Uuid> {
        self.iter()
            .find(|(_, u)| u.username == username)
            .map(|(id, _)| *id)
    }

    fn try_add(&mut self, user_id: &uuid::Uuid, user: User) -> Result<UserStorageOpStatus> {
        if let Some((_existing_id, existing_user)) =
            self.iter_mut().find(|(_, u)| u.username == user.username)
        {
            if !existing_user.is_deleted {
                return Ok(UserStorageOpStatus::AlreadyExist);
            } else {
                existing_user.is_deleted = false;
                return Ok(UserStorageOpStatus::Updated);
            }
        }

        self.insert(*user_id, user);
        Ok(UserStorageOpStatus::Ok)
    }
    fn delete(&mut self, user_id: &uuid::Uuid) -> Result<()> {
        match self.get_mut(user_id) {
            Some(user) => {
                user.is_deleted = true;
                return Ok(());
            }
            None => {
                return Err(crate::PonyError::Custom(format!(
                    "UserStorage: User not found {}",
                    user_id
                )))
            }
        }
    }

    fn all(&self) -> Result<Vec<(uuid::Uuid, User)>> {
        Ok(self
            .iter()
            .map(|(user_id, user)| (user_id.clone(), user.clone()))
            .collect())
    }
}
