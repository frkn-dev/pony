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
        if uuid::Uuid::parse_str(raw).is_ok() {
            Topic::Init(raw.to_string())
        } else if raw == "all" {
            Topic::All
        } else {
            Topic::Updates(raw.to_string())
        }
    }

    pub fn as_zmq_topic(&self) -> String {
        match self {
            Topic::Updates(env) => env.into(),
            Topic::Init(uuid) => uuid.into(),
            Topic::Unknown(s) => s.clone(),
            Topic::All => "all".to_string(),
        }
    }

    pub fn all(uuid: &uuid::Uuid, env: &str) -> Vec<String> {
        vec![uuid.to_string(), env.to_string(), "all".to_string()]
    }
}
