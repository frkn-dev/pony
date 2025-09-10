use super::super::conn::Conn;
use super::super::proto::Proto;
use crate::zmq::message::Action;
use crate::zmq::message::Message;

pub trait Operations {
    fn get_user_id(&self) -> Option<uuid::Uuid>;
    fn set_user_id(&mut self, user_id: &uuid::Uuid);

    fn get_env(&self) -> String;
    fn set_env(&mut self, env: &str);

    fn as_update_message(&self, conn_id: &uuid::Uuid) -> Message;
    fn as_create_message(&self, conn_id: &uuid::Uuid) -> Message;
    fn as_delete_message(&self, conn_id: &uuid::Uuid) -> Message;
}

impl Operations for Conn {
    fn get_env(&self) -> String {
        self.env.clone()
    }

    fn get_user_id(&self) -> Option<uuid::Uuid> {
        self.user_id.clone()
    }
    fn set_user_id(&mut self, user_id: &uuid::Uuid) {
        self.user_id = Some(*user_id);
    }
    fn set_env(&mut self, env: &str) {
        self.env = env.to_string();
    }

    fn as_create_message(&self, conn_id: &uuid::Uuid) -> Message {
        let password = match &self.proto {
            Proto::Shadowsocks { password } => Some(password.clone()),
            _ => None,
        };

        let tag = self.proto.proto();

        let wg = match &self.proto {
            Proto::Wireguard { param, .. } => Some(param.clone()),
            _ => None,
        };

        Message {
            conn_id: (*conn_id).into(),
            action: Action::Create,
            password,
            tag: tag,
            wg,
        }
    }

    fn as_update_message(&self, conn_id: &uuid::Uuid) -> Message {
        let password = match &self.proto {
            Proto::Shadowsocks { password } => Some(password.clone()),
            _ => None,
        };

        let tag = self.proto.proto();

        let wg = match &self.proto {
            Proto::Wireguard { param, .. } => Some(param.clone()),
            _ => None,
        };

        Message {
            conn_id: (*conn_id).into(),
            action: Action::Update,
            password,
            tag: tag,
            wg,
        }
    }

    fn as_delete_message(&self, conn_id: &uuid::Uuid) -> Message {
        let password = match &self.proto {
            Proto::Shadowsocks { password } => Some(password.clone()),
            _ => None,
        };

        let tag = self.proto.proto();

        let wg = match &self.proto {
            Proto::Wireguard { param, .. } => Some(param.clone()),
            _ => None,
        };

        Message {
            conn_id: (*conn_id).into(),
            action: Action::Delete,
            password,
            tag: tag,
            wg,
        }
    }
}
