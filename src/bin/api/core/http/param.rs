use serde::{Deserialize, Serialize};

use super::request::TagReq;

use pony::memory::{key::Code, tag::ProtoTag as Tag};

fn default_format() -> String {
    "plain".to_string()
}

fn default_env() -> String {
    "dev".to_string()
}

fn default_proto() -> TagReq {
    TagReq::Xray
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubIdQueryParam {
    pub id: uuid::Uuid,
    pub env: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubQueryParam {
    pub id: uuid::Uuid,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_env")]
    pub env: String,
    #[serde(default = "default_proto")]
    pub proto: TagReq,
}

#[derive(Debug, Deserialize)]

pub struct NodesQueryParams {
    pub env: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeIdParam {
    pub id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnQueryParam {
    pub id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnTypeParam {
    pub proto: Tag,
    pub last_update: Option<u64>,
    pub env: Option<String>,
    pub topic: String,
}

#[derive(Serialize, Deserialize)]
pub struct KeyQueryParams {
    pub key: Code,
}
