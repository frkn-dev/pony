use super::Tag;
use log::debug;
use std::fmt;
use tonic::{Request, Status};
use uuid::Uuid;

use super::{client::XrayClients, user};
use crate::xray_api::xray::app::stats::command::{GetStatsRequest, GetStatsResponse};

#[derive(Debug, Clone)]
pub enum Stat {
    User(StatType),
    Inbound(StatType),
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stat::User(StatType::Uplink) => write!(f, "uplink"),
            Stat::User(StatType::Downlink) => write!(f, "downlink"),
            Stat::User(StatType::Online) => write!(f, "online"),
            Stat::Inbound(StatType::Uplink) => write!(f, "uplink"),
            Stat::Inbound(StatType::Downlink) => write!(f, "downlink"),
            Stat::Inbound(StatType::Online) => write!(f, "Not implemented"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatType {
    Uplink,
    Downlink,
    Online,
}

impl fmt::Display for StatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatType::Uplink => write!(f, "uplink"),
            StatType::Downlink => write!(f, "downlink"),
            StatType::Online => write!(f, "online"),
        }
    }
}

pub struct UserStats {
    pub downlink: i64,
    pub uplink: i64,
    pub online: i64,
}

pub struct NodeStats {
    pub downlink: i64,
    pub uplink: i64,
}

#[derive(Debug, Clone)]
pub enum Prefix {
    UserPrefix(Uuid),
    InboundPrefix(Tag),
}

async fn get_stat(
    clients: XrayClients,
    prefix: Prefix,
    stat_type: Stat,
    reset: bool,
) -> Result<GetStatsResponse, Status> {
    let response = {
        let mut client = clients.stats_client.lock().await;

        let stat_name = match prefix {
            Prefix::InboundPrefix(tag) => format!("inbound>>>{}", tag),
            Prefix::UserPrefix(uuid) => format!("user>>>{}@pony", uuid),
        };

        match stat_type {
            Stat::User(StatType::Downlink) | Stat::User(StatType::Uplink) => {
                let stat_name = format!("{}>>>traffic>>>{}", stat_name, stat_type);

                let request = Request::new(GetStatsRequest {
                    name: stat_name,
                    reset: reset,
                });
                client.get_stats(request).await
            }
            Stat::User(StatType::Online) => {
                let stat_name = format!("{}>>>{}", stat_name, stat_type);
                let request = Request::new(GetStatsRequest {
                    name: stat_name,
                    reset: reset,
                });
                client.get_stats_online(request).await
            }
            Stat::Inbound(StatType::Downlink) | Stat::Inbound(StatType::Uplink) => {
                let stat_name = format!("{}>>>traffic>>>{}", stat_name, stat_type);

                let request = Request::new(GetStatsRequest {
                    name: stat_name,
                    reset: reset,
                });
                client.get_stats(request).await
            }
            Stat::Inbound(StatType::Online) => {
                Err(Status::internal("Online is not supported for inbound"))
            }
        }
    };

    match response {
        Ok(stat) => Ok(stat.into_inner()),
        Err(e) => Err(Status::internal(format!(
            "Stat request failed {:?} {}",
            stat_type, e
        ))),
    }
}

pub async fn get_user_stats(clients: XrayClients, user_id: Prefix) -> Result<UserStats, Status> {
    debug!("get_user_stats {:?}", user_id);

    let (downlink_result, uplink_result, online_result) = tokio::join!(
        get_stat(
            clients.clone(),
            user_id.clone(),
            Stat::User(StatType::Downlink),
            false
        ),
        get_stat(
            clients.clone(),
            user_id.clone(),
            Stat::User(StatType::Uplink),
            false
        ),
        get_stat(
            clients,
            user_id.clone(),
            Stat::User(StatType::Online),
            false
        )
    );

    match (downlink_result, uplink_result, online_result) {
        (Ok(downlink), Ok(uplink), Ok(online)) => {
            if let (Some(downlink), Some(uplink), Some(online)) = (
                downlink.stat.clone(),
                uplink.stat.clone(),
                online.stat.clone(),
            ) {
                debug!(
                    "User Stats fetched successfully: downlink={:?}, uplink={:?}, online={:?}",
                    downlink.clone(),
                    uplink.clone(),
                    online.clone()
                );
                Ok(UserStats {
                    downlink: downlink.value,
                    uplink: uplink.value,
                    online: online.value,
                })
            } else {
                let error_msg = format!(
                    "Incomplete user stats for {:?}: downlink={:?}, uplink={:?}, online={:?}",
                    user_id,
                    downlink.stat.clone(),
                    uplink.stat.clone(),
                    online.stat.clone()
                );
                Err(Status::internal(error_msg))
            }
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            let error_msg = format!(
                "Failed to fetch user stats for {:?}: error={:?}",
                user_id, e
            );
            Err(Status::internal(error_msg))
        }
    }
}

pub async fn get_inbound_stats(clients: XrayClients, inbound: Prefix) -> Result<NodeStats, Status> {
    let downlink_result = get_stat(
        clients.clone(),
        inbound.clone(),
        Stat::Inbound(StatType::Downlink),
        false,
    )
    .await;

    let uplink_result = get_stat(
        clients.clone(),
        inbound.clone(),
        Stat::Inbound(StatType::Uplink),
        false,
    )
    .await;

    match (downlink_result, uplink_result) {
        (Ok(downlink), Ok(uplink)) => {
            if let (Some(downlink), Some(uplink)) = (downlink.stat.clone(), uplink.stat.clone()) {
                debug!(
                    "Node Stats successfully fetched: inbound={:?}, downlink={:?}, uplink={:?}",
                    inbound, downlink, uplink
                );
                Ok(NodeStats {
                    downlink: downlink.value,
                    uplink: uplink.value,
                })
            } else {
                let error_msg = format!(
                    "Incomplete stats for inbound {:?}: downlink={:?}, uplink={:?}",
                    inbound,
                    downlink.stat.clone(),
                    uplink.stat.clone()
                );
                Err(Status::internal(error_msg))
            }
        }

        (Err(e1), Err(e2)) => {
            let error_msg = format!(
                "Both requests failed for inbound {:?}: downlink error: {:?}, uplink error: {:?}",
                inbound, e1, e2
            );
            Err(Status::internal(error_msg))
        }
        (Err(e), _) | (_, Err(e)) => {
            let error_msg = format!(
                "One of the requests failed for inbound {:?}: {:?}",
                inbound, e
            );
            Err(Status::internal(error_msg))
        }
    }
}

pub async fn get_user_count(clients: XrayClients, inbound: Tag) -> Result<i64, Status> {
    match user::user_count(clients, inbound.clone()).await {
        Ok(count) => Ok(count),
        Err(e) => Err(Status::internal(format!(
            "Failed to fetch user count for inbound {}: {}",
            inbound, e
        ))),
    }
}
