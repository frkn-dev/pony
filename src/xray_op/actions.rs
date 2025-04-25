use std::error::Error;
use std::sync::Arc;

use log::{debug, error};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{client::HandlerClient, shadowsocks, vless, vmess};
use crate::xray_op::vless::UserFlow;
use crate::ProtocolUser;

pub async fn create_users(
    user_id: Uuid,
    password: Option<String>,
    client: Arc<Mutex<HandlerClient>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut users: Vec<Box<dyn ProtocolUser>> = vec![];

    // VMess
    users.push(Box::new(vmess::UserInfo::new(user_id)));

    // VLESS Vision
    users.push(Box::new(vless::UserInfo::new(user_id, UserFlow::Vision)));

    // VLESS Direct
    users.push(Box::new(vless::UserInfo::new(user_id, UserFlow::Direct)));
    // Shadowsocks
    if let Some(pass) = password.clone() {
        users.push(Box::new(shadowsocks::UserInfo::new(user_id, Some(pass))));
    }

    for user in users {
        if let Err(e) = user.send(client.clone()).await {
            error!("Failed to create user for tag {:?}: {}", user.tag(), e);
        } else {
            debug!("Successfully created user for tag {:?}", user.tag());
        }
    }

    Ok(())
}

pub async fn remove_users(
    user_id: Uuid,
    client: Arc<Mutex<HandlerClient>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let user_info = vmess::UserInfo::new(user_id);
    user_info.remove(client.clone()).await?;

    let user_info = vless::UserInfo::new(user_id, UserFlow::Vision);
    user_info.remove(client.clone()).await?;

    let user_info = vless::UserInfo::new(user_id, UserFlow::Direct);
    user_info.remove(client.clone()).await?;

    let user_info = shadowsocks::UserInfo::new(user_id, Some("password".to_string()));
    user_info.remove(client.clone()).await?;

    Ok(())
}
