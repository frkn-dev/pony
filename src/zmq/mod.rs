pub mod message;
pub mod publisher;
pub mod subscriber;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Topic {
    Updates(String),
    Init(String),
    Unknown(String),
    All,
}

impl Topic {
    pub fn from_raw(raw: &str) -> Self {
        if let Ok(_) = uuid::Uuid::parse_str(raw) {
            Topic::Init(raw.to_string())
        } else if raw == "all" {
            Topic::All
        } else {
            Topic::Updates(raw.to_string())
        }
    }

    pub fn as_zmq_topic(&self) -> String {
        match self {
            Topic::Updates(env) => format!("{env}"),
            Topic::Init(uuid) => format!("{uuid}"),
            Topic::Unknown(s) => s.clone(),
            Topic::All => "all".to_string(),
        }
    }

    pub fn all(uuid: &uuid::Uuid, env: &str) -> Vec<String> {
        vec![format!("{uuid}"), format!("{env}"), "all".to_string()]
    }
}
