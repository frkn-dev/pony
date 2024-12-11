use chrono::{DateTime, Utc};
use log::error;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tonic::Request;

use super::{client::XrayClients, Tag};
use crate::xray_api::xray::app::proxyman::command::{
    GetInboundUserRequest, GetInboundUserResponse, GetInboundUsersCountResponse,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    Active,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub user_id: String,
    pub trial: bool,
    pub limit: i64,
    pub status: UserStatus,
    pub uplink: Option<i64>,
    pub downlink: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub modified_at: Option<DateTime<Utc>>,
    pub proto: Option<Vec<Tag>>,
    pub password: String,
}

impl User {
    pub fn new(user_id: String, limit: i64, trial: bool, password: String) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            limit,
            trial,
            status: UserStatus::Active,
            uplink: None,
            downlink: None,
            created_at: now,
            modified_at: None,
            proto: None,
            password: password,
        }
    }

    pub fn update_modified_at(&mut self) {
        self.modified_at = Some(Utc::now());
    }

    pub fn reset_downlink(&mut self) {
        self.downlink = Some(0);
    }

    pub fn reset_uplink(&mut self) {
        self.uplink = Some(0);
    }

    pub fn add_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            if !proto.contains(&tag) {
                proto.push(tag);
            }
        } else {
            self.proto = Some(vec![tag]);
        }
    }

    pub fn remove_proto(&mut self, tag: Tag) {
        if let Some(proto) = &mut self.proto {
            proto.retain(|p| *p != tag);
            if proto.is_empty() {
                self.proto = None;
            }
        }
    }

    pub fn has_proto_tag(&self, tag: Tag) -> bool {
        if let Some(proto_tags) = &self.proto {
            return proto_tags.contains(&tag);
        }
        false
    }
}

pub async fn get_user(
    clients: XrayClients,
    tag: Tag,
    user_id: String,
) -> Result<GetInboundUserResponse, tonic::Status> {
    let request = GetInboundUserRequest {
        tag: tag.to_string(),
        email: format!("{}@pony", user_id).to_string(),
    };

    let mut handler_client = clients.handler_client.lock().await;

    handler_client
        .get_inbound_users(Request::new(request))
        .await
        .map(|res| res.into_inner())
        .map_err(|e| {
            error!("Failed to fetch users for tag {}: {}", tag, e);
            e
        })
}

pub async fn user_count(
    clients: XrayClients,
    tag: Tag,
) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let request = GetInboundUserRequest {
        tag: tag.to_string(),
        email: "".to_string(),
    };

    let mut handler_client = clients.handler_client.lock().await;

    match handler_client
        .get_inbound_users_count(Request::new(request))
        .await
    {
        Ok(res) => {
            let res: GetInboundUsersCountResponse = res.into_inner();
            Ok(res.count)
        }
        Err(e) => {
            error!("Failed to fetch users for tag {}: {}", tag, e);
            Err(Box::new(e))
        }
    }
}
