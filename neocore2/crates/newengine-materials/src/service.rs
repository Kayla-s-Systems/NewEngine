#![forbid(unsafe_op_in_unsafe_fn)]

use crate::api::{MaterialDescriptor, MaterialId, MaterialTextureBindings};
use crate::texture_refs::MaterialTextureReference;

pub const ENGINE_MATERIALS_SERVICE_ID: &str = "engine.materials";
pub const MATERIALS_SERVICE_ID: &str = "materials.api";
pub const MATERIALS_BACKEND_CAPABILITY_ID: &str = "materials.backend";

pub const MATERIALS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "materials",
        ENGINE_MATERIALS_SERVICE_ID,
        MATERIALS_SERVICE_ID,
        MATERIALS_BACKEND_CAPABILITY_ID,
    );

pub mod method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const LOAD_JSON_V1: &str = "materials.load_json_v1";
    pub const DESCRIBE_TEXTURE_REF_JSON_V1: &str = "materials.describe_texture_ref_json_v1";
    pub const FORMATS_JSON_V1: &str = "materials.formats_json_v1";
}

pub const MATERIALS_SERVICE_METHODS: &[&str] = &[
    method::INFO_JSON,
    method::INVOKE_JSON,
    method::SHUTDOWN_V1,
    method::LOAD_JSON_V1,
    method::DESCRIBE_TEXTURE_REF_JSON_V1,
    method::FORMATS_JSON_V1,
];

pub const MATERIALS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_MATERIALS_SERVICE_ID,
        "newengine.materials-api >= 0.1.x",
        MATERIALS_SERVICE_METHODS,
    );

pub const MATERIALS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        MATERIALS_RUNTIME_CONTRACT_SPEC,
        Some(MATERIALS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_MATERIALS_BACKEND"),
    );

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialLoadRequest {
    pub logical_path: String,
}

impl Default for MaterialLoadRequest {
    fn default() -> Self {
        Self { logical_path: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialLoadResponse {
    pub source: String,
    pub name: String,
    pub id: MaterialId,
    pub descriptor: MaterialDescriptor,
    pub textures: MaterialTextureBindings,
}

impl Default for MaterialLoadResponse {
    fn default() -> Self {
        Self {
            source: String::new(),
            name: String::new(),
            id: MaterialId::invalid(),
            descriptor: MaterialDescriptor::default(),
            textures: MaterialTextureBindings::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialTextureRefRequest {
    pub reference: String,
}

impl Default for MaterialTextureRefRequest {
    fn default() -> Self {
        Self { reference: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialTextureRefInfo {
    pub valid: bool,
    pub canonical: String,
    pub dictionary_path: String,
    pub entry_selector: String,
}

impl Default for MaterialTextureRefInfo {
    fn default() -> Self {
        Self {
            valid: false,
            canonical: String::new(),
            dictionary_path: String::new(),
            entry_selector: String::new(),
        }
    }
}

impl MaterialTextureRefInfo {
    pub fn from_reference(value: &str) -> Self {
        match MaterialTextureReference::parse(value) {
            Some(reference) => Self {
                valid: true,
                canonical: reference.canonical,
                dictionary_path: reference.dictionary_path,
                entry_selector: reference.entry_selector,
            },
            None => Self { canonical: value.trim().replace('\\', "/"), ..Self::default() },
        }
    }
}
