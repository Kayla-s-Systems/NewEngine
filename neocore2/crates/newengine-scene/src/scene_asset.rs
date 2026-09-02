#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;

use serde::{Deserialize, Serialize};

use newengine_ecs::{EntityId, World};
use newengine_transform::set_parent;
use newengine_transform_api::{Parent, Transform};

use crate::components::{ActiveCamera, DefinitionRef, EntityGuid, Name, SceneRoot};
use crate::guid::{ensure_entity_guid, GuidAllocator};
use crate::settings::SceneSettings;
use crate::Scene;

pub const SCENE_ASSET_SCHEMA_V1: &str = "newengine.scene.asset.v1";
pub const SCENE_ASSET_STATUS_TRANSITIONAL_JSON: &str = "transitional_json_scene_asset";

mod serde_u128_string {
    use serde::{Deserialize, Deserializer, Serializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum GuidValue {
        Text(String),
        Unsigned(u64),
        Signed(i64),
    }

    fn parse_text(value: &str) -> Result<u128, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("GUID string is empty".to_owned());
        }
        let compact = value.strip_prefix("0x").unwrap_or(value).replace('-', "");
        if compact.len() == 32 || value.starts_with("0x") || value.contains('-') {
            return u128::from_str_radix(&compact, 16)
                .map_err(|error| format!("invalid hexadecimal GUID '{value}': {error}"));
        }
        value
            .parse::<u128>()
            .or_else(|_| u128::from_str_radix(&compact, 16))
            .map_err(|error| format!("invalid GUID '{value}': {error}"))
    }

    fn into_u128<E: serde::de::Error>(value: GuidValue) -> Result<u128, E> {
        match value {
            GuidValue::Text(value) => parse_text(&value).map_err(E::custom),
            GuidValue::Unsigned(value) => Ok(u128::from(value)),
            GuidValue::Signed(value) if value >= 0 => Ok(value as u128),
            GuidValue::Signed(value) => Err(E::custom(format!("GUID cannot be negative: {value}"))),
        }
    }

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{value:032x}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        into_u128(GuidValue::deserialize(deserializer)?)
    }

    pub(super) fn deserialize_option<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<GuidValue>::deserialize(deserializer)?
            .map(into_u128)
            .transpose()
    }
}

mod serde_option_u128_string {
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<u128>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_some(&format!("{value:032x}")),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        super::serde_u128_string::deserialize_option(deserializer)
    }
}

/// Options for extracting a `SceneAsset` from a runtime scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneAssetOptions {
    /// If true, include entities that have no `Name` and no `Transform`.
    pub include_empty_entities: bool,
}

impl Default for SceneAssetOptions {
    #[inline]
    fn default() -> Self {
        Self {
            include_empty_entities: false,
        }
    }
}

/// Serializable scene representation.
///
/// This is a **content** format, not a runtime format:
/// - entities are identified by `EntityGuid` (stable)
/// - runtime-only identifiers (`EntityId`) are not stored
/// - component payloads are intentionally minimal and extendable
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneAsset {
    pub schema: String,
    pub version: u32,

    pub settings: SceneSettings,

    /// GUID allocator state to keep authoring deterministic.
    pub guid_seed: u64,
    pub guid_next: u64,

    #[serde(default, with = "serde_option_u128_string")]
    pub root: Option<u128>,
    #[serde(default, with = "serde_option_u128_string")]
    pub active_camera: Option<u128>,

    pub entities: Vec<SceneEntityAsset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneEntityAsset {
    #[serde(with = "serde_u128_string")]
    pub guid: u128,
    pub name: Option<String>,
    #[serde(default, with = "serde_option_u128_string")]
    pub parent: Option<u128>,
    pub transform: Option<TransformAsset>,
    /// Optional reference to a .ytyp Definition Entry consumed by scene placement.
    /// Scene stores the placement reference only; .ytyp metadata is owned by engine.assets.definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TransformAsset {
    pub position: [f32; 3],
    /// Quaternion (x, y, z, w)
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl From<Transform> for TransformAsset {
    #[inline]
    fn from(t: Transform) -> Self {
        Self {
            position: [t.position.x, t.position.y, t.position.z],
            rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
            scale: [t.scale.x, t.scale.y, t.scale.z],
        }
    }
}

impl TransformAsset {
    #[inline]
    pub fn into_transform(self) -> Transform {
        Transform {
            position: newengine_math::Vec3::new(
                self.position[0],
                self.position[1],
                self.position[2],
            ),
            rotation: newengine_math::Quat::from_xyzw(
                self.rotation[0],
                self.rotation[1],
                self.rotation[2],
                self.rotation[3],
            ),
            scale: newengine_math::Vec3::new(self.scale[0], self.scale[1], self.scale[2]),
        }
    }
}

#[derive(Debug)]
pub enum SceneAssetError {
    MissingGuidAllocator,
    InvalidRootGuid(u128),
    InvalidActiveCameraGuid(u128),
}

impl fmt::Display for SceneAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGuidAllocator => write!(f, "scene asset: missing guid allocator"),
            Self::InvalidRootGuid(g) => write!(f, "scene asset: invalid root guid {g}"),
            Self::InvalidActiveCameraGuid(g) => {
                write!(f, "scene asset: invalid active camera guid {g}")
            }
        }
    }
}

impl std::error::Error for SceneAssetError {}

impl Scene {
    /// Extracts a serializable `SceneAsset` from the current runtime scene.
    ///
    /// Note: this guarantees every entity in the asset has an `EntityGuid`.
    pub fn to_asset(&mut self, opts: SceneAssetOptions) -> SceneAsset {
        // Ensure invariants and ensure GUIDs for entities.
        let _ = self.validate_invariants();

        let world = self.world_mut();

        // Pass 1: ensure GUID for every entity.
        let ids: Vec<EntityId> = world.iter_entities().collect();
        for id in ids.iter().copied() {
            let _ = ensure_entity_guid(world, id);
        }

        // Helper maps: native EntityId stays local to the scene runtime; parent links
        // cross the component boundary as opaque EntityHandle stable ids.
        let mut id_to_guid: newengine_math::collections::FxHashMap<EntityId, u128> =
            Default::default();
        let mut handle_to_guid: newengine_math::collections::FxHashMap<u64, u128> =
            Default::default();
        for id in world.iter_entities() {
            if let Some(g) = world.get::<EntityGuid>(id) {
                id_to_guid.insert(id, g.0);
                handle_to_guid.insert(id.stable_u64(), g.0);
            }
        }

        // Root/camera GUIDs.
        let root = world
            .query::<SceneRoot>()
            .next()
            .and_then(|(id, _)| id_to_guid.get(&id).copied());

        let active_camera = world
            .query::<ActiveCamera>()
            .next()
            .and_then(|(id, _)| id_to_guid.get(&id).copied());

        // Allocator state.
        let alloc = *world.resource_mut_or_insert_default::<GuidAllocator>();

        // Entities.
        let mut entities: Vec<SceneEntityAsset> = Vec::with_capacity(world.entity_count());

        for id in world.iter_entities() {
            let guid = match world.get::<EntityGuid>(id) {
                Some(g) => g.0,
                None => continue,
            };

            let name = world.get::<Name>(id).map(|n| n.0.clone());
            let transform = world
                .get::<Transform>(id)
                .copied()
                .map(TransformAsset::from);

            let parent = world
                .get::<Parent>(id)
                .and_then(|p| handle_to_guid.get(&p.0.stable_id).copied());

            let definition_ref = world.get::<DefinitionRef>(id).map(|r| r.0.clone());

            if !opts.include_empty_entities
                && name.is_none()
                && transform.is_none()
                && definition_ref.is_none()
            {
                // Skip entities that carry no authoring signal.
                continue;
            }

            entities.push(SceneEntityAsset {
                guid,
                name,
                parent,
                transform,
                definition_ref,
            });
        }

        // Deterministic order: sort by guid.
        entities.sort_by_key(|entity| entity.guid);

        SceneAsset {
            schema: SCENE_ASSET_SCHEMA_V1.to_string(),
            version: 1,
            settings: self.settings,
            guid_seed: alloc.seed,
            guid_next: alloc.next,
            root,
            active_camera,
            entities,
        }
    }

    /// Replaces this scene with the instantiated contents of `asset`.
    ///
    /// Instantiation creates new runtime `EntityId`s but preserves stable `EntityGuid`s.
    pub fn load_asset(&mut self, asset: &SceneAsset) -> Result<(), SceneAssetError> {
        let mut world = World::new();

        // Restore allocator state.
        world.insert_resource(GuidAllocator {
            seed: asset.guid_seed,
            next: asset.guid_next,
        });

        // Pass 1: spawn all entities and map guid -> EntityId.
        let mut guid_to_id: newengine_math::collections::FxHashMap<u128, EntityId> =
            Default::default();
        for e in asset.entities.iter() {
            let id = world.spawn();
            let _ = world.insert(id, EntityGuid(e.guid));
            guid_to_id.insert(e.guid, id);

            if let Some(name) = e.name.as_ref() {
                let _ = world.insert(id, Name(name.clone()));
            }
            if let Some(t) = e.transform {
                let _ = world.insert(id, t.into_transform());
            }
            if let Some(definition_ref) = e
                .definition_ref
                .as_ref()
                .map(|it| it.trim())
                .filter(|it| !it.is_empty())
            {
                let _ = world.insert(id, DefinitionRef(definition_ref.to_owned()));
            }
        }

        // Pass 2: apply hierarchy.
        for e in asset.entities.iter() {
            let Some(&id) = guid_to_id.get(&e.guid) else {
                continue;
            };
            let parent_id = e.parent.and_then(|pg| guid_to_id.get(&pg).copied());
            let _ = set_parent(&mut world, id, parent_id);
        }

        // Markers: root/camera.
        if let Some(root_g) = asset.root {
            let Some(&id) = guid_to_id.get(&root_g) else {
                return Err(SceneAssetError::InvalidRootGuid(root_g));
            };
            let _ = world.insert(id, SceneRoot);
        }

        if let Some(cam_g) = asset.active_camera {
            let Some(&id) = guid_to_id.get(&cam_g) else {
                return Err(SceneAssetError::InvalidActiveCameraGuid(cam_g));
            };
            let _ = world.insert(id, ActiveCamera);
        }

        // Swap into self.
        self.world = world;
        self.settings = asset.settings;
        let _ = self.validate_invariants();

        Ok(())
    }
}

#[cfg(test)]
mod json_guid_tests {
    use super::*;

    fn asset_with_guid(guid: u128) -> SceneAsset {
        SceneAsset {
            schema: SCENE_ASSET_SCHEMA_V1.to_owned(),
            version: 1,
            settings: SceneSettings::default(),
            guid_seed: 1,
            guid_next: 2,
            root: Some(guid),
            active_camera: None,
            entities: vec![SceneEntityAsset {
                guid,
                name: Some("large-guid".to_owned()),
                parent: None,
                transform: None,
                definition_ref: None,
            }],
        }
    }

    #[test]
    fn scene_asset_json_roundtrips_full_u128_guid_as_string() {
        let guid = u128::MAX - 41;
        let asset = asset_with_guid(guid);
        let text = serde_json::to_string(&asset).expect("serialize scene asset");
        assert!(text.contains(&format!("{guid:032x}")));
        let restored = serde_json::from_str::<SceneAsset>(&text).expect("deserialize scene asset");
        assert_eq!(restored.root, Some(guid));
        assert_eq!(restored.entities[0].guid, guid);
    }

    #[test]
    fn scene_asset_json_accepts_legacy_small_numeric_guids() {
        let mut value = serde_json::to_value(asset_with_guid(7)).expect("serialize fixture");
        value["root"] = serde_json::json!(7);
        value["entities"][0]["guid"] = serde_json::json!(7);
        let restored = serde_json::from_value::<SceneAsset>(value).expect("legacy numeric guid");
        assert_eq!(restored.root, Some(7));
        assert_eq!(restored.entities[0].guid, 7);
    }
}
