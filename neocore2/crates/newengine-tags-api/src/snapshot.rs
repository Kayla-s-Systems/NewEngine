use newengine_entity_api::EntityHandle;
use serde::{Deserialize, Serialize};

use crate::TagId;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagSetSnapshotV1 {
    pub owner: String,
    #[serde(default)]
    pub entity: Option<EntityHandle>,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub source: String,
}
