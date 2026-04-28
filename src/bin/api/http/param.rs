use serde::{Deserialize, Serialize};

use pony::{Code, Tag};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubIdQueryParam {
    pub id: uuid::Uuid,
    pub env: Option<String>,
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
