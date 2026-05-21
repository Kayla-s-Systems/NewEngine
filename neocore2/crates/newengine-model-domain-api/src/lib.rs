#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable domain contracts for model construction.
//!
//! This crate owns the engine-facing gateway vocabulary and declarative DTOs
//! shared by model runtime providers, player/NPC construction and future plugin
//! overrides. It does not parse OBJ/YMT/MTL/NEYTD and it does not access the
//! host or AssetManager directly.

use serde::{Deserialize, Serialize};

pub use newengine_model_skeleton_api::{
    ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
};

pub const ENGINE_MODEL_SERVICE_ID: &str = "engine.model";
pub const ENGINE_MODEL_SKELETONS_SERVICE_ID: &str = "engine.model.skeletons";
pub const ENGINE_MODEL_MATERIALS_SERVICE_ID: &str = "engine.model.materials";
pub const ENGINE_MODEL_COLLISIONS_SERVICE_ID: &str = "engine.model.collisions";

pub const MODEL_SERVICE_ID: &str = "model.api";
pub const MODEL_BACKEND_CAPABILITY_ID: &str = "model.backend";
pub const MODEL_SKELETONS_SERVICE_ID: &str = "model.skeletons.api";
pub const MODEL_SKELETONS_BACKEND_CAPABILITY_ID: &str = "model.skeletons.backend";
pub const MODEL_MATERIALS_SERVICE_ID: &str = "model.materials.api";
pub const MODEL_MATERIALS_BACKEND_CAPABILITY_ID: &str = "model.materials.backend";
pub const MODEL_COLLISIONS_SERVICE_ID: &str = "model.collisions.api";
pub const MODEL_COLLISIONS_BACKEND_CAPABILITY_ID: &str = "model.collisions.backend";

pub const MODEL_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const MODEL_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const MODEL_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1: &str = "assemble_json_v1";
pub const MODEL_SERVICE_METHOD_VALIDATE_JSON_V1: &str = "validate_json_v1";


pub const DRAWABLE_DICTIONARY_EXTENSION: &str = "ydd";
pub const DRAWABLE_DICTIONARY_ASSET_KIND: &str = "drawable_dictionary";
pub const DRAWABLE_DICTIONARY_MAGIC: [u8; 4] = *b"RSC7";

pub const MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1: &str = "model.drawable_dictionary_manifest_json_v1";
pub const OBJECT_TYPE_DEFINITIONS_EXTENSION: &str = "ytyp";
pub const OBJECT_TYPE_DEFINITIONS_ASSET_KIND: &str = "object_type_definitions";
pub const OBJECT_TYPE_DEFINITIONS_CONTAINER: &str = "rockstar.map_types.ytyp.xml";
pub const MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1: &str = "model.definition_entries_json_v1";

pub const MODEL_FEATURE_DOMAINS: &[&str] = &[
    "mesh.obj",
    "material.mtl",
    "skeleton.rsc7",
    "collision.default",
    "drawable.ydd",
    "definition_entries.ytyp",
];


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryRequest {
    pub source: String,
    pub selector: Option<String>,
}

impl Default for DrawableDictionaryRequest {
    fn default() -> Self {
        Self { source: String::new(), selector: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryEntry {
    pub name: String,
    pub name_hash: u64,
    pub mesh_count: u32,
    pub material_slots: Vec<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl Default for DrawableDictionaryEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            name_hash: 0,
            mesh_count: 0,
            material_slots: Vec::new(),
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryManifest {
    pub schema: String,
    pub source: String,
    pub asset_kind: String,
    pub container: String,
    pub texture_dictionary: Option<String>,
    pub entries: Vec<DrawableDictionaryEntry>,
}

impl Default for DrawableDictionaryManifest {
    fn default() -> Self {
        Self {
            schema: "newengine.drawable_dictionary.manifest.v1".to_owned(),
            source: String::new(),
            asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(),
            container: "rockstar.drawable_dictionary.rsc7".to_owned(),
            texture_dictionary: None,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntriesRequest {
    pub source: String,
    pub selector: Option<String>,
}

impl Default for DefinitionEntriesRequest {
    fn default() -> Self {
        Self { source: String::new(), selector: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionDictionaries {
    pub texture: Option<String>,
    pub drawable: Option<String>,
    pub clip: Option<String>,
    pub physics: Option<String>,
}

impl Default for DefinitionDictionaries {
    fn default() -> Self {
        Self { texture: None, drawable: None, clip: None, physics: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionBounds {
    pub bb_min: [f32; 3],
    pub bb_max: [f32; 3],
    pub bs_centre: [f32; 3],
    pub bs_radius: f32,
}

impl Default for DefinitionBounds {
    fn default() -> Self {
        Self { bb_min: [0.0; 3], bb_max: [0.0; 3], bs_centre: [0.0; 3], bs_radius: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntry {
    pub entry_kind: String,
    pub name: String,
    pub asset_name: String,
    pub asset_type: String,
    pub lod_dist: f32,
    pub hd_texture_dist: f32,
    pub flags: u32,
    pub special_attribute: u32,
    pub bounds: DefinitionBounds,
    pub dictionaries: DefinitionDictionaries,
}

impl Default for DefinitionEntry {
    fn default() -> Self {
        Self {
            entry_kind: "CBaseArchetypeDef".to_owned(),
            name: String::new(),
            asset_name: String::new(),
            asset_type: String::new(),
            lod_dist: 0.0,
            hd_texture_dist: 0.0,
            flags: 0,
            special_attribute: 0,
            bounds: DefinitionBounds::default(),
            dictionaries: DefinitionDictionaries::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntriesManifest {
    pub schema: String,
    pub codec: String,
    pub source_format: String,
    pub source: String,
    pub name: String,
    pub definition_entries: Vec<DefinitionEntry>,
}

impl Default for DefinitionEntriesManifest {
    fn default() -> Self {
        Self {
            schema: "newengine.ytyp.definition_entries.v1".to_owned(),
            codec: "asset.codec.ytyp".to_owned(),
            source_format: OBJECT_TYPE_DEFINITIONS_CONTAINER.to_owned(),
            source: String::new(),
            name: String::new(),
            definition_entries: Vec::new(),
        }
    }
}

pub const MODEL_SERVICE_METHODS: &[&str] = &[
    MODEL_SERVICE_METHOD_INFO,
    MODEL_SERVICE_METHOD_INVOKE,
    MODEL_SERVICE_METHOD_SHUTDOWN_V1,
    MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
    MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1,
];

pub const MODEL_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model",
        ENGINE_MODEL_SERVICE_ID,
        MODEL_SERVICE_ID,
        MODEL_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_SKELETONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model.skeletons",
        ENGINE_MODEL_SKELETONS_SERVICE_ID,
        MODEL_SKELETONS_SERVICE_ID,
        MODEL_SKELETONS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_MATERIALS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model.materials",
        ENGINE_MODEL_MATERIALS_SERVICE_ID,
        MODEL_MATERIALS_SERVICE_ID,
        MODEL_MATERIALS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_COLLISIONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model.collisions",
        ENGINE_MODEL_COLLISIONS_SERVICE_ID,
        MODEL_COLLISIONS_SERVICE_ID,
        MODEL_COLLISIONS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_MODEL_SERVICE_ID,
        "newengine.model-domain-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

pub const MODEL_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        MODEL_RUNTIME_CONTRACT_SPEC,
        Some(MODEL_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_MODEL_BACKEND"),
    );

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelAssetRequest {
    pub model: String,
    pub manifest: Option<String>,
    pub skeleton: Option<String>,
    pub texture_dictionary: Option<String>,
    pub collisions: Vec<ModelCollisionRef>,
    pub target_height: f32,
    pub eye_height_ratio: f32,
}

impl Default for ModelAssetRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            manifest: None,
            skeleton: None,
            texture_dictionary: None,
            collisions: Vec::new(),
            target_height: 1.8,
            eye_height_ratio: 0.91,
        }
    }
}

impl ModelAssetRequest {
    #[inline]
    pub fn new(model: impl Into<String>) -> Self {
        Self { model: model.into(), ..Self::default() }
    }

    #[inline]
    pub fn with_manifest(mut self, manifest: impl Into<String>) -> Self {
        self.manifest = Some(manifest.into());
        self
    }

    #[inline]
    pub fn with_skeleton(mut self, skeleton: impl Into<String>) -> Self {
        self.skeleton = Some(skeleton.into());
        self
    }

    #[inline]
    pub fn with_texture_dictionary(mut self, dictionary: impl Into<String>) -> Self {
        self.texture_dictionary = Some(dictionary.into());
        self
    }

    #[inline]
    pub fn with_collision(mut self, collision: ModelCollisionRef) -> Self {
        self.collisions.push(collision);
        self
    }

    #[inline]
    pub fn with_human_scale(mut self, target_height: f32, eye_height_ratio: f32) -> Self {
        self.target_height = target_height;
        self.eye_height_ratio = eye_height_ratio;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelAssetBundle {
    pub source: String,
    pub parts: Vec<ModelMeshPart>,
    pub skeleton: Option<ModelSkeletonMetadata>,
    pub texture_dictionary: Option<String>,
    pub collisions: Vec<ModelCollisionRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelMeshPart {
    pub material_slot: String,
    pub mesh: newengine_primitives::PrimitiveMesh,
    pub material: ModelMaterialBinding,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMaterialBinding {
    pub slot: String,
    pub descriptor: newengine_materials::MaterialDescriptor,
    pub textures: newengine_materials::MaterialTextureBindings,
    pub fallback_color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConstructionManifest {
    pub id: String,
    pub model: String,
    pub skeleton: Option<ModelSkeletonRef>,
    pub material_set: ModelMaterialSetRef,
    pub collisions: Vec<ModelCollisionRef>,
    pub target_height: f32,
    pub eye_height_ratio: f32,
}

impl Default for ModelConstructionManifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            model: String::new(),
            skeleton: None,
            material_set: ModelMaterialSetRef::default(),
            collisions: Vec::new(),
            target_height: 1.8,
            eye_height_ratio: 0.91,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelSkeletonRef {
    pub source: String,
    pub format: String,
    pub humanoid_profile: Option<String>,
}

impl Default for ModelSkeletonRef {
    fn default() -> Self {
        Self { source: String::new(), format: "auto".to_owned(), humanoid_profile: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelMaterialSetRef {
    pub texture_dictionary: Option<String>,
    pub material_manifest: Option<String>,
}

impl Default for ModelMaterialSetRef {
    fn default() -> Self { Self { texture_dictionary: None, material_manifest: None } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelCollisionRef {
    pub name: String,
    pub kind: ModelCollisionKind,
    pub anchor: Option<String>,
    pub radius: f32,
    pub half_height: f32,
    pub half_extents: [f32; 3],
    pub mesh: Option<String>,
}

impl Default for ModelCollisionRef {
    fn default() -> Self {
        Self {
            name: "body".to_owned(),
            kind: ModelCollisionKind::Capsule,
            anchor: Some("hips".to_owned()),
            radius: 0.32,
            half_height: 0.82,
            half_extents: [0.32, 0.82, 0.32],
            mesh: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCollisionKind {
    None,
    Capsule,
    Box,
    Sphere,
    Mesh,
}

impl Default for ModelCollisionKind {
    fn default() -> Self { Self::Capsule }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelConstructionValidation {
    pub valid: bool,
    pub resolved: Option<ModelAssetRequest>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_gateway_first() {
        assert_eq!(MODEL_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_MODEL_SERVICE_ID);
        assert_eq!(MODEL_SKELETONS_BACKEND_SERVICE_SPEC.domain, "model.skeletons");
        assert_eq!(MODEL_MATERIALS_BACKEND_SERVICE_SPEC.domain, "model.materials");
        assert_eq!(MODEL_COLLISIONS_BACKEND_SERVICE_SPEC.domain, "model.collisions");
        assert!(MODEL_FEATURE_DOMAINS.contains(&"definition_entries.ytyp"));
    }
}
