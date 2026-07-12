use serde::{Deserialize, Serialize};

use crate::EntityHandle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityListRequest {
    #[serde(default = "default_entity_limit")]
    pub limit: usize,
}

impl Default for EntityListRequest {
    #[inline]
    fn default() -> Self {
        Self {
            limit: default_entity_limit(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub handle: EntityHandle,
    #[serde(default)]
    pub lifecycle: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub debug_identity: String,
}

impl EntityRecord {
    #[inline]
    pub fn alive(handle: EntityHandle) -> Self {
        Self {
            handle,
            lifecycle: "alive".to_owned(),
            tags: Vec::new(),
            owner: None,
            debug_identity: format!("entity:{}", handle.stable_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityListResponse {
    #[serde(default)]
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityExistsRequest {
    pub entity: EntityHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityExistsResponse {
    pub entity: EntityHandle,
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySpawnRequest {
    #[serde(default = "default_spawn_count")]
    pub count: usize,
}

impl Default for EntitySpawnRequest {
    #[inline]
    fn default() -> Self {
        Self {
            count: default_spawn_count(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntitySpawnResponse {
    #[serde(default)]
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDespawnRequest {
    #[serde(default)]
    pub entities: Vec<EntityHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDespawnResult {
    pub entity: EntityHandle,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityDespawnResponse {
    pub ok: bool,
    #[serde(default)]
    pub results: Vec<EntityDespawnResult>,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityInvokeRequest {
    /// One of `list_json_v1`, `exists_json_v1`, `spawn_json_v1`, `despawn_json_v1`.
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[inline]
fn default_entity_limit() -> usize {
    4096
}

#[inline]
fn default_spawn_count() -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_request_decodes_with_default_limit() {
        let request: EntityListRequest = serde_json::from_str("{}").expect("defaults decode");
        assert_eq!(request.limit, 4096);
    }

    #[test]
    fn spawn_request_decodes_with_default_count() {
        let request: EntitySpawnRequest = serde_json::from_str("{}").expect("defaults decode");
        assert_eq!(request.count, 1);
    }

    #[test]
    fn alive_record_populates_diagnostic_fields() {
        let record = EntityRecord::alive(EntityHandle::new(91));
        assert_eq!(record.lifecycle, "alive");
        assert_eq!(record.debug_identity, "entity:91");
    }
}
