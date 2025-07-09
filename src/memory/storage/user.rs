use chrono::Utc;
use std::collections::HashMap;

use super::super::storage::Status as OperationStatus;
use super::super::user::User;
use crate::error::Result;
use crate::http::requests::UserUpdateReq;

pub trait Operations {
    fn by_username(&self, username: &str) -> Option<uuid::Uuid>;
    fn all(&self) -> Result<Vec<(uuid::Uuid, User)>>;
    fn delete(&mut self, user_id: &uuid::Uuid) -> OperationStatus;
    fn update(&mut self, user_id: &uuid::Uuid, user_req: UserUpdateReq) -> OperationStatus;
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

    fn update(&mut self, user_id: &uuid::Uuid, user_req: UserUpdateReq) -> OperationStatus {
        match self.get_mut(user_id) {
            Some(user) => {
                if user_req.is_deleted.is_none() && user.is_deleted {
                    return OperationStatus::NotFound(*user_id);
                }

                let mut changed = false;

                if let Some(deleted) = user_req.is_deleted {
                    if !user.is_deleted && deleted {
                        return OperationStatus::NotModified(*user_id);
                    }
                    if user.is_deleted != deleted {
                        user.is_deleted = deleted;
                        changed = true;
                    }
                }
                if let Some(env) = user_req.env {
                    if user.env != env {
                        user.env = env;
                        changed = true;
                    }
                }
                if let Some(password) = user_req.password {
                    if user.password.as_ref() != Some(&password) {
                        user.password = Some(password);
                        changed = true;
                    }
                }
                if let Some(limit) = user_req.limit {
                    if user.limit != Some(limit) {
                        user.limit = Some(limit);
                        changed = true;
                    }
                }
                if changed {
                    user.modified_at = Utc::now().naive_utc();
                    OperationStatus::Updated(*user_id)
                } else {
                    OperationStatus::NotModified(*user_id)
                }
            }
            None => OperationStatus::NotFound(*user_id),
        }
    }
}
