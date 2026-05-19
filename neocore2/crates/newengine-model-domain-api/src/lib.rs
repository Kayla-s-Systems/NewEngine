#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable domain contracts for model construction.
//!
//! This crate does not parse OBJ/YMT/MTL/NEYTD itself. It defines the engine
//! gateway vocabulary and declarative DTOs used by adapters/providers that
//! assemble a runtime model from mesh, skeleton, material and collision sources.

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_gateway_first() {
        assert_eq!(MODEL_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_MODEL_SERVICE_ID);
        assert_eq!(MODEL_SKELETONS_BACKEND_SERVICE_SPEC.domain, "model.skeletons");
        assert_eq!(MODEL_MATERIALS_BACKEND_SERVICE_SPEC.domain, "model.materials");
        assert_eq!(MODEL_COLLISIONS_BACKEND_SERVICE_SPEC.domain, "model.collisions");
    }
}
