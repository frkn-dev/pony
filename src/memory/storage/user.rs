use std::collections::HashMap;

use super::super::storage::Status as OperationStatus;
use super::super::user::User;
use crate::error::Result;

pub trait Operations {
    fn by_username(&self, username: &str) -> Option<uuid::Uuid>;
    fn all(&self) -> Result<Vec<(uuid::Uuid, User)>>;
    fn delete(&mut self, user_id: &uuid::Uuid) -> OperationStatus;
}

impl Operations for HashMap<uuid::Uuid, User> {
    fn by_username(&self, username: &str) -> Option<uuid::Uuid> {
        self.iter()
            .find(|(_, u)| u.username == username)
            .map(|(id, _)| *id)
    }

    fn all(&self) -> Result<Vec<(uuid::Uuid, User)>> {
        Ok(self
            .iter()
            .map(|(user_id, user)| (*user_id, user.clone()))
            .collect())
    }

    fn delete(&mut self, user_id: &uuid::Uuid) -> OperationStatus {
        if let Some(user) = self.get_mut(user_id) {
            if user.is_deleted {
                return OperationStatus::NotFound(*user_id);
            }
            user.is_deleted = true;
            OperationStatus::Ok(*user_id)
        } else {
            OperationStatus::NotFound(*user_id)
        }
    }
}
