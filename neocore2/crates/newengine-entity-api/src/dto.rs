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
    pub archetype: Option<String>,
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
            archetype: None,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySpawnTransform {
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default = "default_spawn_rotation")]
    pub rotation_xyzw: [f32; 4],
    #[serde(default = "default_spawn_scale")]
    pub scale: [f32; 3],
}

impl Default for EntitySpawnTransform {
    #[inline]
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation_xyzw: default_spawn_rotation(),
            scale: default_spawn_scale(),
        }
    }
}

/// Provider-neutral entity construction request.
///
/// `archetype` selects a registered composition factory. `properties` are opaque to the entity
/// gateway and interpreted by that factory; transform/tags/owner are common lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySpawnRequest {
    #[serde(default = "default_spawn_count")]
    pub count: usize,
    #[serde(default = "default_archetype_id")]
    pub archetype: String,
    #[serde(default)]
    pub transform: Option<EntitySpawnTransform>,
    #[serde(default)]
    pub properties: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,
}

impl Default for EntitySpawnRequest {
    #[inline]
    fn default() -> Self {
        Self {
            count: default_spawn_count(),
            archetype: default_archetype_id(),
            transform: None,
            properties: serde_json::Value::Null,
            tags: Vec::new(),
            owner: None,
        }
    }
}

/// Authored archetype definition layered on top of a concrete composition factory.
///
/// Concrete factories remain Rust/plugin mechanisms (`player.fps`, `entity.empty`, future
/// vehicle/NPC compositions). Game content registers authored ids that inherit a base factory
/// and provide defaults without adding a new Rust constructor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityArchetypeDefinition {
    pub id: String,
    pub base_archetype: String,
    pub owner: String,
    pub description: String,
    pub definition_ref: Option<String>,
    pub default_properties: serde_json::Value,
    pub tags: Vec<String>,
    pub default_owner: Option<String>,
}

impl Default for EntityArchetypeDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            base_archetype: "entity.empty".to_owned(),
            owner: "authored".to_owned(),
            description: String::new(),
            definition_ref: None,
            default_properties: serde_json::Value::Object(serde_json::Map::new()),
            tags: Vec::new(),
            default_owner: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityArchetypeDefinitionIdRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityArchetypeDefinitionMutationResponse {
    pub ok: bool,
    pub id: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityArchetypeDescriptor {
    pub id: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source_kind: String,
    #[serde(default)]
    pub base_archetype: Option<String>,
    #[serde(default)]
    pub definition_ref: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityArchetypeListResponse {
    #[serde(default)]
    pub archetypes: Vec<EntityArchetypeDescriptor>,
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
fn default_archetype_id() -> String {
    "entity.empty".to_owned()
}

#[inline]
fn default_spawn_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

#[inline]
fn default_spawn_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
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
        assert_eq!(request.archetype, "entity.empty");
    }

    #[test]
    fn alive_record_populates_diagnostic_fields() {
        let record = EntityRecord::alive(EntityHandle::new(91));
        assert_eq!(record.lifecycle, "alive");
        assert_eq!(record.debug_identity, "entity:91");
    }
}
