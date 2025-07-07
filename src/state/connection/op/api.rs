use crate::state::connection::conn::Conn;
use crate::state::connection::conn::Status;
use crate::zmq::message::Action;
use crate::zmq::message::Message;
use crate::Proto;

pub trait Operations {
    fn get_trial(&self) -> bool;
    fn set_trial(&mut self, v: bool);

    fn get_limit(&self) -> i32;
    fn set_limit(&mut self, v: i32);

    fn get_status(&self) -> Status;
    fn set_status(&mut self, s: Status);

    fn get_user_id(&self) -> Option<uuid::Uuid>;
    fn set_user_id(&mut self, user_id: &uuid::Uuid);

    fn get_env(&self) -> String;
    fn set_env(&mut self, env: &str);

    fn as_update_message(&self, conn_id: &uuid::Uuid) -> Message;
    fn as_create_message(&self, conn_id: &uuid::Uuid) -> Message;
    fn as_delete_message(&self, conn_id: &uuid::Uuid) -> Message;
}

impl Operations for Conn {
    fn get_trial(&self) -> bool {
        self.trial
    }
    fn set_trial(&mut self, v: bool) {
        self.trial = v;
    }

    fn get_limit(&self) -> i32 {
        self.limit
    }
    fn set_limit(&mut self, v: i32) {
        self.limit = v;
    }

    fn get_status(&self) -> Status {
        self.status.clone()
    }
    fn set_status(&mut self, s: Status) {
        self.status = s;
    }

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
            conn_id: *conn_id,
            action: Action::Create,
            password,
            tag,
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
            conn_id: *conn_id,
            action: Action::Update,
            password,
            tag,
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
            conn_id: *conn_id,
            action: Action::Delete,
            password,
            tag,
            wg,
        }
    }
}
