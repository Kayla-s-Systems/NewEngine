#![forbid(unsafe_op_in_unsafe_fn)]

use crate::api::{MaterialDescriptor, MaterialId, MaterialTextureBindings};
use crate::texture_refs::MaterialTextureReference;

pub const ENGINE_ASSETS_MATERIALS_SERVICE_ID: &str = "engine.assets.materials";
pub const MATERIALS_SERVICE_ID: &str = "materials.api";
pub const MATERIALS_BACKEND_CAPABILITY_ID: &str = "assets.materials.backend";

pub const MATERIALS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.materials",
        ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        MATERIALS_SERVICE_ID,
        MATERIALS_BACKEND_CAPABILITY_ID,
    );

pub mod method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const LOAD_JSON_V1: &str = "assets.materials.load_json_v1";
    pub const DESCRIBE_TEXTURE_REF_JSON_V1: &str = "assets.materials.describe_texture_ref_json_v1";
    pub const FORMATS_JSON_V1: &str = "assets.materials.formats_json_v1";
    pub const MANIFEST_JSON_V1: &str = "assets.materials.manifest_v1";
    pub const LOAD_DESCRIPTOR_V1: &str = "assets.materials.load_descriptor_v1";
    pub const RESOLVE_GRAPH_V1: &str = "assets.materials.resolve_graph_v1";
    pub const VALIDATE_V1: &str = "assets.materials.validate_v1";
    pub const TO_RENDER_PACKET_V1: &str = "assets.materials.to_render_packet_v1";
}

pub const MATERIALS_SERVICE_METHODS: &[&str] = &[
    method::INFO_JSON,
    method::INVOKE_JSON,
    method::SHUTDOWN_V1,
    method::LOAD_JSON_V1,
    method::DESCRIBE_TEXTURE_REF_JSON_V1,
    method::FORMATS_JSON_V1,
    method::MANIFEST_JSON_V1,
    method::LOAD_DESCRIPTOR_V1,
    method::RESOLVE_GRAPH_V1,
    method::VALIDATE_V1,
    method::TO_RENDER_PACKET_V1,
];

pub const MATERIALS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        "newengine.assets.materials-api >= 0.1.x",
        MATERIALS_SERVICE_METHODS,
    );

pub const MATERIALS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        MATERIALS_RUNTIME_CONTRACT_SPEC,
        Some(MATERIALS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_MATERIALS_BACKEND"),
    );

#[derive(Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialLoadRequest {
    /// First-class material selector. Preferred form is `materials/foo.nemat@entry`.
    pub logical_path: String,
    /// Optional split selector for hosts that pass path and entry separately.
    pub selector: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialTextureRefRequest {
    pub reference: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialTextureRefInfo {
    pub valid: bool,
    pub canonical: String,
    pub dictionary_path: String,
    pub entry_selector: String,
    pub extension: String,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}
impl MaterialTextureRefInfo {
    pub fn from_reference(value: &str) -> Self {
        match MaterialTextureReference::parse_strict(value) {
            Ok(reference) => Self {
                valid: true,
                canonical: reference.canonical,
                dictionary_path: reference.dictionary_path,
                entry_selector: reference.entry_selector,
                extension: "ytd".to_owned(),
                warnings: Vec::new(),
                errors: Vec::new(),
            },
            Err(error) => Self {
                canonical: value.trim().replace('\\', "/"),
                errors: vec![error],
                ..Self::default()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialsManifest {
    pub schema: String,
    pub gateway: String,
    pub primary_format: String,
    pub texture_reference_syntax: String,
    pub methods: Vec<String>,
    pub policy: Vec<String>,
}

impl Default for MaterialsManifest {
    fn default() -> Self {
        Self {
            schema: "newengine.assets.materials.manifest.v1".to_owned(),
            gateway: ENGINE_ASSETS_MATERIALS_SERVICE_ID.to_owned(),
            primary_format: "nemat".to_owned(),
            texture_reference_syntax: "<logical-path>.ytd@entry".to_owned(),
            methods: MATERIALS_SERVICE_METHODS
                .iter()
                .map(|m| (*m).to_owned())
                .collect(),
            policy: vec![
                "engine.assets.materials is the only material resolve gateway".to_owned(),
                "materials reference .ytd@entry texture dictionaries".to_owned(),
                "renderer receives RenderMaterialPacket only".to_owned(),
                "raw image paths outside authored ListFile dictionaries are invalid".to_owned(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialDescriptorLoadResponse {
    pub source: String,
    pub name: String,
    pub descriptor: MaterialDescriptor,
    pub textures: MaterialTextureBindings,
}
#[derive(Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialValidationRequest {
    pub logical_path: String,
    pub selector: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialValidationResult {
    pub valid: bool,
    pub source: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ResolvedMaterialGraph {
    pub schema: String,
    pub source: String,
    pub name: String,
    pub shader: String,
    pub descriptor: MaterialDescriptor,
    pub textures: MaterialTextureBindings,
    pub texture_refs: Vec<MaterialTextureRefInfo>,
    pub warnings: Vec<String>,
}
impl Default for ResolvedMaterialGraph {
    fn default() -> Self {
        Self {
            schema: "newengine.assets.materials.resolved_graph.v1".to_owned(),
            source: String::new(),
            name: String::new(),
            shader: "pbr.default".to_owned(),
            descriptor: MaterialDescriptor::default(),
            textures: MaterialTextureBindings::default(),
            texture_refs: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct RenderMaterialPacket {
    pub schema: String,
    pub source: String,
    pub name: String,
    pub descriptor: MaterialDescriptor,
    pub textures: MaterialTextureBindings,
    pub packet_kind: String,
}
impl Default for RenderMaterialPacket {
    fn default() -> Self {
        Self {
            schema: "newengine.assets.materials.render_packet.v1".to_owned(),
            source: String::new(),
            name: String::new(),
            descriptor: MaterialDescriptor::default(),
            textures: MaterialTextureBindings::default(),
            packet_kind: "renderer_agnostic_material_packet".to_owned(),
        }
    }
}
