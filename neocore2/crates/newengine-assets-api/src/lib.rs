#![forbid(unsafe_op_in_unsafe_fn)]

use core::time::Duration;

mod asset_error;
pub use asset_error::*;

/// Engine-facing asset service gateway id.
///
/// Runtime and provider plugins must request assets through this stable host-owned
/// gateway, not through a concrete AssetManager/provider service id. The host
/// resolves this gateway to the active asset backend by declared capability.
pub const ENGINE_ASSET_SERVICE_ID: &str = "engine.assets";

/// Canonical client-facing service id for asset access.
pub const ASSET_SERVICE_ID: &str = ENGINE_ASSET_SERVICE_ID;

/// Default provider service id used by the first-party AssetManager backend.
///
/// This is provider-owned identity, not the id consumers should call. Third-party
/// providers may register a different service id as long as they declare
/// `asset_manager.backend`; the engine gateway still resolves them.
pub const ASSET_PROVIDER_SERVICE_ID: &str = "asset_manager.api";

/// Backend capability declared by plugins that provide an asset service backend.
pub const ASSET_BACKEND_CAPABILITY_ID: &str = "asset_manager.backend";

/// Wire method namespace for asset-domain service calls.
pub const ASSET_METHOD_PREFIX: &str = "asset.";

/// Semantic texture dictionary runtime gateway id. File-type descriptors route `.ytd`
/// meaning here. `engine.assets.textures` owns validation, manifest semantics and runtime
/// texture packets; `engine.assets` remains the byte/VFS/codec-dispatch owner.
pub const ENGINE_ASSETS_TEXTURES_SERVICE_ID: &str = "engine.assets.textures";
pub const TEXTURES_SERVICE_ID: &str = "textures.api";
pub const TEXTURES_BACKEND_CAPABILITY_ID: &str = "assets.textures.backend";
pub const TEXTURES_RUNTIME_CONTRACT: &str = "newengine.assets.textures.runtime.v1";
/// Semantic definition/archetype metadata gateway id. File-type descriptors route
/// `.ytyp` meaning here; scene/world systems may consume definitions later, but
/// do not own the file type.
pub const ENGINE_ASSETS_DEFINITIONS_SERVICE_ID: &str = "engine.assets.definitions";
pub const DEFINITIONS_SERVICE_ID: &str = "definitions.api";
pub const DEFINITIONS_BACKEND_CAPABILITY_ID: &str = "assets.definitions.backend";
pub const DEFINITIONS_RUNTIME_CONTRACT: &str = "newengine.assets.definitions.runtime.v1";
pub const ENGINE_ASSETS_MODELS_SERVICE_ID: &str = "engine.assets.models";
pub const ENGINE_ASSETS_MATERIALS_SERVICE_ID: &str = "engine.assets.materials";
/// Semantic authored map/world placement gateway id. `.ymap` owns map composition,
/// placements and references to `.ytyp` Definition Entries; it does not replace
/// `.ytyp` as the generic metadata/knowledge source.
pub const ENGINE_ASSETS_MAPS_SERVICE_ID: &str = "engine.assets.maps";

/// Semantic UI dictionary gateway id. `.neui` meaning lives here: XMLcentral validation,
/// entry selection, binding/action/dependency extraction and authored-to-runtime DTO
/// compilation. `engine.assets` remains byte/VFS/codec owner; `engine.ui` remains live runtime.
pub const ENGINE_ASSETS_UI_SERVICE_ID: &str = "engine.assets.ui";
pub const ASSETS_UI_SERVICE_ID: &str = "assets.ui.api";
pub const ASSETS_UI_BACKEND_CAPABILITY_ID: &str = "assets.ui.backend";
pub const ASSETS_UI_RUNTIME_CONTRACT: &str = "newengine.assets.ui.runtime.v1";

/// Runtime scene gateway. It consumes resolved map/definition DTOs and mutates the world; it does not own authored map file semantics.
pub const ENGINE_SCENE_SERVICE_ID: &str = "engine.scene";

/// Runtime scripting gateway. `.ysc` script module entries are opaque to core and
/// are routed through this domain; AssetManager still owns VFS bytes and ListFile codec dispatch.
pub const ENGINE_SCRIPTING_SERVICE_ID: &str = "engine.scripting";

/// Semantic asset graph gateway id. This resolver owns declarative dependency
/// graph expansion over .ytyp/.ydd/.nemat/.ytd refs; it uses engine.assets only
/// for VFS bytes and codec dispatch.
pub const ENGINE_ASSETS_GRAPH_SERVICE_ID: &str = "engine.assets.graph";
pub const ASSET_GRAPH_SERVICE_ID: &str = "asset_graph.api";
pub const ASSET_GRAPH_BACKEND_CAPABILITY_ID: &str = "assets.graph.backend";
pub const ASSET_GRAPH_RUNTIME_CONTRACT: &str = "newengine.assets.graph.runtime.v1";


/// Editor/import lifecycle sub-gateways and capability ids.
///
/// These are Godot-inspired lifecycle surfaces, but they do not adopt Godot's
/// `.tres/.res` resource model. `engine.assets` remains the byte/VFS/codec host;
/// these sub-gateways expose inspectable editor/import read-model slices.
pub const ENGINE_ASSETS_UID_SERVICE_ID: &str = "engine.assets.uid";
pub const ASSETS_UID_SERVICE_ID: &str = "assets.uid.api";
pub const ASSETS_UID_BACKEND_CAPABILITY_ID: &str = "assets.uid.backend";
pub const ASSETS_UID_RUNTIME_CONTRACT: &str = "newengine.assets.uid.v1";

pub const ENGINE_ASSETS_DEPENDENCIES_SERVICE_ID: &str = "engine.assets.dependencies";
pub const ASSETS_DEPENDENCIES_SERVICE_ID: &str = "assets.dependencies.api";
pub const ASSETS_DEPENDENCIES_BACKEND_CAPABILITY_ID: &str = "assets.dependencies.backend";
pub const ASSETS_DEPENDENCIES_RUNTIME_CONTRACT: &str = "newengine.assets.dependencies.v1";

pub const ENGINE_ASSETS_IMPORT_QUEUE_SERVICE_ID: &str = "engine.assets.import_queue";
pub const ASSETS_IMPORT_QUEUE_SERVICE_ID: &str = "assets.import_queue.api";
pub const ASSETS_IMPORT_QUEUE_BACKEND_CAPABILITY_ID: &str = "assets.import_queue.backend";
pub const ASSETS_IMPORT_QUEUE_RUNTIME_CONTRACT: &str = "newengine.assets.import_queue.v1";

pub const ENGINE_ASSETS_PACKAGE_WRITER_SERVICE_ID: &str = "engine.assets.package_writer";
pub const ASSETS_PACKAGE_WRITER_SERVICE_ID: &str = "assets.package_writer.api";
pub const ASSETS_PACKAGE_WRITER_CAPABILITY_ID: &str = "assets.package_writer";
pub const ASSETS_PACKAGE_WRITER_RUNTIME_CONTRACT: &str = "newengine.assets.package_writer.v1";

pub const ASSETS_REIMPORT_CAPABILITY_ID: &str = "assets.reimport";
pub const ASSETS_THUMBNAIL_CAPABILITY_ID: &str = "assets.thumbnail";
pub const ASSETS_DIRTY_SCAN_CAPABILITY_ID: &str = "assets.dirty_scan";
pub const ASSETS_IMPORT_LIFECYCLE_CAPABILITY_ID: &str = "assets.import_lifecycle";

pub const ASSETS_UID_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.uid",
        ENGINE_ASSETS_UID_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSETS_UID_BACKEND_CAPABILITY_ID,
    );

pub const ASSETS_DEPENDENCIES_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.dependencies",
        ENGINE_ASSETS_DEPENDENCIES_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSETS_DEPENDENCIES_BACKEND_CAPABILITY_ID,
    );

pub const ASSETS_IMPORT_QUEUE_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.import_queue",
        ENGINE_ASSETS_IMPORT_QUEUE_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSETS_IMPORT_QUEUE_BACKEND_CAPABILITY_ID,
    );

pub const ASSETS_PACKAGE_WRITER_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.package_writer",
        ENGINE_ASSETS_PACKAGE_WRITER_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSETS_PACKAGE_WRITER_CAPABILITY_ID,
    );

/// Asset streaming gateway and capability ids.
///
/// Streaming is a first-class diagnostics/jobs-visible domain, not a hidden
/// loader thread behind AssetManager. Providers should expose request queues,
/// residency, defragmentation and cache loading through this capability family.
pub const ENGINE_ASSETS_STREAMING_SERVICE_ID: &str = "engine.assets.streaming";
pub const ASSETS_STREAMING_SERVICE_ID: &str = "assets.streaming.api";
pub const ASSETS_STREAMING_BACKEND_CAPABILITY_ID: &str = "assets.streaming.backend";
pub const ASSETS_STREAMING_REQUEST_QUEUE_CAPABILITY_ID: &str = "assets.streaming.request_queue";
pub const ASSETS_STREAMING_RESIDENCY_CAPABILITY_ID: &str = "assets.streaming.residency";
pub const ASSETS_STREAMING_DEFRAG_CAPABILITY_ID: &str = "assets.streaming.defrag";
pub const ASSETS_STREAMING_CACHE_LOADER_CAPABILITY_ID: &str = "assets.streaming.cache_loader";
pub const ASSETS_STREAMING_RUNTIME_CONTRACT: &str = "newengine.assets.streaming.runtime.v1";

/// World streaming gateway and capability ids.
///
/// World streaming owns cell visibility and spatial budget decisions. It may
/// request asset streaming work, but all loader/visibility jobs must remain on
/// the engine job system and visible in diagnostics/profiler.
pub const ENGINE_WORLD_STREAMING_SERVICE_ID: &str = "engine.world.streaming";
pub const WORLD_STREAMING_SERVICE_ID: &str = "world.streaming.api";
pub const WORLD_STREAMING_BACKEND_CAPABILITY_ID: &str = "world.streaming.backend";
pub const WORLD_STREAMING_CELLS_CAPABILITY_ID: &str = "world.streaming.cells";
pub const WORLD_STREAMING_VISIBILITY_BUDGET_CAPABILITY_ID: &str = "world.streaming.visibility_budget";
pub const WORLD_STREAMING_RUNTIME_CONTRACT: &str = "newengine.world.streaming.runtime.v1";

pub mod asset_graph_method {
    pub const RESOLVE_V1: &str = "assets.graph.resolve_v1";
    pub const VALIDATE_V1: &str = "assets.graph.validate_v1";
    pub const DUMP_JSON_V1: &str = "assets.graph.dump_json_v1";
}

pub const ASSET_GRAPH_SERVICE_METHODS: &[&str] = &[
    newengine_service_api::SERVICE_METHOD_INFO_JSON,
    newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
    asset_graph_method::RESOLVE_V1,
    asset_graph_method::VALIDATE_V1,
    asset_graph_method::DUMP_JSON_V1,
];

pub mod assets_ui_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const MANIFEST_V1: &str = "assets.ui.manifest_v1";
    pub const REGISTRY_V1: &str = "assets.ui.registry_v1";
    pub const ENTRY_V1: &str = "assets.ui.entry_v1";
    pub const DOCUMENT_V1: &str = "assets.ui.document_v1";
    pub const COMPILE_DOCUMENT_V1: &str = "assets.ui.compile_document_v1";
    pub const BINDING_PLAN_V1: &str = "assets.ui.binding_plan_v1";
    pub const VALIDATE_V1: &str = "assets.ui.validate_v1";
    pub const DEPENDENCIES_V1: &str = "assets.ui.dependencies_v1";
    pub const DUMP_XMLCENTRAL_V1: &str = "assets.ui.dump_xmlcentral_v1";
}

pub const ASSETS_UI_SERVICE_METHODS: &[&str] = &[
    assets_ui_method::INFO_JSON,
    assets_ui_method::INVOKE_JSON,
    assets_ui_method::SHUTDOWN_V1,
    assets_ui_method::MANIFEST_V1,
    assets_ui_method::REGISTRY_V1,
    assets_ui_method::ENTRY_V1,
    assets_ui_method::DOCUMENT_V1,
    assets_ui_method::COMPILE_DOCUMENT_V1,
    assets_ui_method::BINDING_PLAN_V1,
    assets_ui_method::VALIDATE_V1,
    assets_ui_method::DEPENDENCIES_V1,
    assets_ui_method::DUMP_XMLCENTRAL_V1,
];

mod asset_ref;
pub use asset_ref::*;
pub mod list_file;
pub use list_file::*;

/// Generic host/plugin backend declaration for the asset service family.
pub const ASSET_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets",
        ENGINE_ASSET_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSET_BACKEND_CAPABILITY_ID,
    );

/// Engine-facing file-type registry gateway id.
///
/// This is a descriptor/navigation surface over runtime asset containers. It does
/// not replace `engine.assets`; registered file-type handlers still read payloads
/// through the AssetManager/VFS gateway.
pub const ENGINE_ASSET_FILE_TYPES_SERVICE_ID: &str = "engine.assets.file_types";
pub const ASSET_FILE_TYPES_SERVICE_ID: &str = "asset.file_types.api";
pub const ASSET_FILE_TYPES_BACKEND_CAPABILITY_ID: &str = "assets.file_types.backend";

pub const ASSET_FILE_TYPES_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.file_types",
        ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
        ASSET_FILE_TYPES_SERVICE_ID,
        ASSET_FILE_TYPES_BACKEND_CAPABILITY_ID,
    );

pub mod file_type_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const MANIFEST_JSON_V1: &str = "asset.file_types.manifest_json_v1";
    pub const REGISTER_JSON_V1: &str = "asset.file_types.register_json_v1";
    pub const PROBE_JSON_V1: &str = "asset.file_types.probe_json_v1";
    pub const RESOLVE_JSON_V1: &str = "asset.file_types.resolve_json_v1";
}

pub const ASSET_FILE_TYPE_SERVICE_METHODS: &[&str] = &[
    file_type_method::INFO_JSON,
    file_type_method::INVOKE_JSON,
    file_type_method::SHUTDOWN_V1,
    file_type_method::MANIFEST_JSON_V1,
    file_type_method::REGISTER_JSON_V1,
    file_type_method::PROBE_JSON_V1,
    file_type_method::RESOLVE_JSON_V1,
];

pub const ASSET_FILE_TYPES_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
        "newengine.assets-file-types-api >= 0.1.x",
        ASSET_FILE_TYPE_SERVICE_METHODS,
    );

pub const ASSET_FILE_TYPES_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ASSET_FILE_TYPES_RUNTIME_CONTRACT_SPEC,
        Some(ASSET_FILE_TYPES_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ASSET_FILE_TYPES"),
    );

/// Asset codec classification used by AssetManager to apply generic host rules.
///
/// The host does not interpret concrete formats. It only enforces broad safety
/// constraints declared by the codec provider. Encoding/compression is not part
/// of this enum; codecs own their internal source envelope and may support
/// raw XML, native binary, deflate, etc. behind the same logical descriptor.
pub mod codec_type {
    /// Container codec. May expose nested VFS entries and may recursively host
    /// other assets. Example: .nepak.
    pub const CONTAINER: &str = "containerType";
    /// List codec. A single file contains multiple same-domain records selected
    /// by name/hash/index, but it cannot host nested assets. Examples: domain dictionaries projected from NEF8 entries.
    pub const LIST: &str = "listType";
    /// Canonical NEF8 ListFile binary envelope. The file extension remains domain-facing
    /// (`.ytyp`, `.ytd`, `.ydd`, `.nemat`) while the header content_kind selects the payload domain.
    pub const LIST_FILE: &str = "listFile";
    /// Single binary file with magic bytes and one decoded object. Not used for `.nemat`, which is a NEF8 material library.
    pub const SINGLE: &str = "singleType";
    /// Asset definition metadata. It is not tied to a text encoding: the same
    /// logical format may be XML today, binary tomorrow, or compressed binary
    /// later. Example: .ytyp Definition Entries.
    pub const DEFINITION: &str = "definitionType";
    /// Plain UTF-8 text without magic bytes. Example: future .bindings.json codec.
    pub const PLAIN_TEXT: &str = "plainText";
}

#[inline]
pub fn codec_type_allows_nested_assets(codec_type: &str) -> bool {
    codec_type.trim().eq_ignore_ascii_case(codec_type::CONTAINER)
}

#[inline]
pub fn codec_type_requires_magic_by_default(codec_type: &str) -> bool {
    !codec_type.trim().eq_ignore_ascii_case(codec_type::PLAIN_TEXT)
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetFileTypeDescriptor {
    pub extension: String,
    pub asset_kind: String,
    pub container: String,
    /// Optional provider-declared NEF8/ListFile content kind. The registry stores
    /// this as descriptor data only; core must not derive semantic routing from it.
    pub content_kind: Option<u32>,
    /// Broad codec class. AssetManager uses this only for generic restrictions:
    /// nested VFS is allowed for `containerType` and forbidden for every other kind.
    pub codec_type: String,
    /// Owner of byte access, VFS/package mount and codec dispatch. For normal
    /// runtime assets this is `engine.assets`.
    pub byte_owner: String,
    /// Gateway that owns semantic interpretation of decoded entries.
    pub semantic_gateway: String,
    /// Compatibility projection for older descriptor consumers. It mirrors
    /// `semantic_gateway` and must not be used as the byte owner.
    pub gateway: String,
    pub handler_service: String,
    pub read_method: String,
    pub selector_syntax: Option<String>,
    pub consumer_domains: Vec<String>,
    /// Hex-encoded magic bytes. Required for magic-routed binary codecs, optional
    /// for codecs that deliberately own extension/source-policy routing such as
    /// `definitionType` authored XML beside future binary envelopes.
    pub magic: Option<String>,
    pub outputs: Vec<String>,
    pub priority: i32,
    pub vfs_backed: bool,
    pub runtime_ready: bool,
    /// True only for codecs that may expose nested VFS entries. This must match
    /// `codec_type == containerType`; mismatches are rejected by the registry.
    pub allow_nested_assets: bool,
    /// Kept as a semantic flag for tooling: the runtime container is native to
    /// NewEngine, not an authoring/source format. It does not grant nesting.
    pub native_container: bool,
    /// Magic is required by default. `plainText` and carefully scoped
    /// `definitionType` codecs may set this to false and identify by
    /// extension/source policy.
    pub requires_magic: bool,
    pub notes: String,
}

impl Default for AssetFileTypeDescriptor {
    fn default() -> Self {
        Self {
            extension: String::new(),
            asset_kind: String::new(),
            container: String::new(),
            content_kind: None,
            codec_type: codec_type::SINGLE.to_owned(),
            byte_owner: ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: String::new(),
            gateway: String::new(),
            handler_service: String::new(),
            read_method: method::DECODE_V1.to_owned(),
            selector_syntax: None,
            consumer_domains: Vec::new(),
            magic: None,
            outputs: Vec::new(),
            priority: 0,
            vfs_backed: true,
            runtime_ready: false,
            allow_nested_assets: false,
            native_container: false,
            requires_magic: true,
            notes: String::new(),
        }
    }
}

impl AssetFileTypeDescriptor {
    pub fn extension_key(extension: &str) -> String {
        extension.trim().trim_start_matches('.').to_ascii_lowercase()
    }

    #[inline]
    pub fn is_container_codec(&self) -> bool {
        codec_type_allows_nested_assets(&self.codec_type)
    }

    #[inline]
    pub fn normalize_layer_contract(&mut self) {
        self.extension = Self::extension_key(&self.extension);
        if self.byte_owner.trim().is_empty() {
            self.byte_owner = ENGINE_ASSET_SERVICE_ID.to_owned();
        }
        // File-type semantics are not inferred here. Each format crate/codec
        // must declare its semantic gateway, handler service and consumers in
        // its own descriptor. The registry is a generic collector/resolver, not
        // a central table of known extensions.
        if self.gateway.trim().is_empty() && !self.semantic_gateway.trim().is_empty() {
            self.gateway = self.semantic_gateway.clone();
        }
        // Keep `gateway` as a semantic projection for descriptor consumers.
        // It must not be used as the byte/VFS owner.
        if self.gateway.trim() != self.semantic_gateway.trim() {
            self.gateway = self.semantic_gateway.clone();
        }
        if self.consumer_domains.is_empty() && !self.semantic_gateway.trim().is_empty() {
            self.consumer_domains = vec![self.semantic_gateway.clone()];
        }
    }

    #[inline]
    pub fn validate_generic_rules(&self) -> Result<(), String> {
        let ext = Self::extension_key(&self.extension);
        if ext.is_empty() {
            return Err("codec descriptor extension is empty".to_owned());
        }
        if self.byte_owner.trim().is_empty() {
            return Err(format!("codec '.{}' descriptor byte_owner is empty", ext));
        }
        if self.semantic_gateway.trim().is_empty() {
            return Err(format!("codec '.{}' descriptor semantic_gateway is empty", ext));
        }
        if self.gateway.trim() != self.semantic_gateway.trim() {
            return Err(format!(
                "codec '.{}' descriptor gateway must mirror semantic_gateway ('{}' != '{}')",
                ext, self.gateway, self.semantic_gateway
            ));
        }
        if self.handler_service.trim().is_empty() {
            return Err(format!("codec '.{}' descriptor handler_service is empty", ext));
        }
        let is_container = self.is_container_codec();
        if self.allow_nested_assets != is_container {
            return Err(format!(
                "codec '.{}' nesting flag mismatch: allow_nested_assets={} codec_type='{}'",
                ext, self.allow_nested_assets, self.codec_type
            ));
        }
        if codec_type_requires_magic_by_default(&self.codec_type) && self.requires_magic && self.magic.is_none() {
            return Err(format!(
                "codec '.{}' is binary type '{}' but declares no magic bytes",
                ext, self.codec_type
            ));
        }
        if !is_container && self.outputs.iter().any(|o| o == "vfs.layer" || o == "container.vfs_layer") {
            return Err(format!(
                "codec '.{}' is '{}' and cannot expose nested VFS outputs",
                ext, self.codec_type
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetFileTypeManifest {
    pub schema: String,
    pub gateway: String,
    pub formats: Vec<AssetFileTypeDescriptor>,
}

impl Default for AssetFileTypeManifest {
    fn default() -> Self {
        Self {
            schema: "newengine.asset_file_types.v2".to_owned(),
            gateway: ENGINE_ASSET_FILE_TYPES_SERVICE_ID.to_owned(),
            formats: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetFileTypeProbeRequest {
    pub logical_path: String,
}

impl Default for AssetFileTypeProbeRequest {
    fn default() -> Self {
        Self { logical_path: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetFileTypeRegisterRequest {
    pub descriptor: AssetFileTypeDescriptor,
}

impl Default for AssetFileTypeRegisterRequest {
    fn default() -> Self {
        Self { descriptor: AssetFileTypeDescriptor::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetDecodeRequest {
    pub logical_path: String,
    pub output_kind: String,
    pub selector: serde_json::Value,
}

impl Default for AssetDecodeRequest {
    fn default() -> Self {
        Self {
            logical_path: String::new(),
            output_kind: String::new(),
            selector: serde_json::Value::Null,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetFileTypeProbeResult {
    pub logical_path: String,
    pub extension: String,
    pub known: bool,
    pub descriptor: Option<AssetFileTypeDescriptor>,
}

impl Default for AssetFileTypeProbeResult {
    fn default() -> Self {
        Self { logical_path: String::new(), extension: String::new(), known: false, descriptor: None }
    }
}

/// Canonical AssetManager v1 method names.
///
/// There is one supported runtime contract: explicit `*_v1` entry points for
/// import/pump/state/text/texture access. Older alias pairs such as
/// `asset.load`, `asset.pump`, and `asset.load_text_v1` are intentionally not
/// part of this surface.
pub mod method {
    /// Standard service-framework metadata method.
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    /// Standard service-framework JSON control invocation method.
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;

    pub const RELOAD_V1: &str = "asset.reload_v1";
    pub const INFO_JSON_V1: &str = "asset.info_json_v1";
    pub const STATE_JSON_V1: &str = "asset.state_json_v1";
    /// Current AssetStatus row by id or logical path. Payload accepts utf8 id_hex32 or logical path.
    pub const STATUS_JSON_V1: &str = "asset.status_json_v1";
    /// Full AssetStatus graph by id or logical path. Payload accepts utf8 id_hex32 or logical path.
    pub const STATUS_GRAPH_JSON_V1: &str = "asset.status_graph_json_v1";
    /// Validated lifecycle projection hook. Payload is JSON with owner/domain/logical_path/stage/proof.
    pub const PROJECT_STATUS_JSON_V1: &str = "asset.project_status_json_v1";
    pub const BLOB_WIRE_V1: &str = "asset.blob_wire_v1";
    /// Runtime-ready RGBA8 texture packet by asset id. AssetManager validates/parses codec metadata.
    pub const TEXTURE_RGBA8_V1: &str = "asset.texture_rgba8_v1";
    /// Generic codec dispatch. Payload is JSON `AssetDecodeRequest`; response is codec-defined bytes.
    pub const DECODE_V1: &str = "asset.decode_v1";
    /// Runtime-ready RGBA8 texture selected from a .ytd dictionary. Payload: JSON { dictionary_path, texture_name | texture_hash }.
    pub const TEXTURE_DICTIONARY_RGBA8_V1: &str = "asset.texture_dictionary_rgba8_v1";
    /// Runtime-ready GPU-native texture selected from a .ytd dictionary.
    /// Returns NTRT v2 with format + complete mip chain. BC1/BC3/BC5/BC7 stay compressed.
    pub const TEXTURE_DICTIONARY_RUNTIME_V1: &str = "asset.texture_dictionary_runtime_v1";
    /// Explicit BCn aliases for callers that want to assert a compressed format class.
    pub const TEXTURE_BC1_V1: &str = "asset.texture_bc1_v1";
    pub const TEXTURE_BC3_V1: &str = "asset.texture_bc3_v1";
    pub const TEXTURE_BC5_V1: &str = "asset.texture_bc5_v1";
    pub const TEXTURE_BC7_V1: &str = "asset.texture_bc7_v1";

    /// Stable v1 import entry point.
    pub const IMPORT_V1: &str = "asset.import_v1";
    /// Stable v1 pump entry point.
    pub const PUMP_V1: &str = "asset.pump_v1";
    /// Raw VFS bytes by logical path. This bypasses codecs but still resolves exclusively through AssetManager mounts.
    pub const RAW_BYTES_V1: &str = "asset.raw_bytes_v1";
    /// Raw UTF-8 text by logical path resolved through AssetManager mounts.
    pub const TEXT_V1: &str = "asset.text_v1";
    // Fast-path / batch APIs.
    pub const PRELOAD_MANY_V1: &str = "asset.preload_many_v1";
    pub const GET_STATE_V1: &str = "asset.get_state_v1";

    pub const FORMATS_JSON_V1: &str = "asset.formats_json_v1";
    pub const SOURCES_JSON_V1: &str = "asset.sources_json_v1";
    pub const VERIFY_ASSETS_JSON_V1: &str = "asset.verify_assets_json_v1";
    pub const SOURCE_KINDS_JSON_V1: &str = "asset.source_kinds_json_v1";
    /// Merged VFS directory listing by logical path. Payload accepts either a UTF-8
    /// logical directory path or JSON { logical_path }. Response is JSON only.
    pub const VFS_LIST_JSON_V1: &str = "asset.vfs_list_json_v1";
    /// Rebuild/repack a NEF8 ListFile after an editor-side entry update/delete/rename.
    /// Payload is JSON and write-back is performed only through a writable VFS source.
    pub const LIST_FILE_REPACK_JSON_V1: &str = "asset.list_file_repack_json_v1";
    pub const MOUNT_SOURCE_JSON_V1: &str = "asset.mount_source_json_v1";

    // Debug/diagnostics.
    pub const RESOLVE_TRACE_JSON_V1: &str = "asset.resolve_trace_json_v1";
    /// Standard listFiles manifest for any dictionary/container asset. Codec-defined output; not a raw VFS read.
    pub const LIST_FILE_MANIFEST_V1: &str = "asset.list_file_manifest_v1";

    // Editor/import lifecycle read-model.
    //
    // These methods deliberately stay under `engine.assets`: the asset backend owns
    // source discovery, UID/cache rows, dirty/reimport state and human-readable
    // diagnostics. Format meaning still belongs to codec/domain gateways, and final
    // editor panels/thumbnails remain UI composition over this data.
    pub const UID_JSON_V1: &str = "asset.uid_json_v1";
    pub const IMPORT_CACHE_JSON_V1: &str = "asset.import_cache_json_v1";
    pub const IMPORT_DIRTY_JSON_V1: &str = "asset.import_dirty_json_v1";
    pub const IMPORT_SCAN_JSON_V1: &str = "asset.import_scan_json_v1";
    pub const IMPORT_GRAPH_JSON_V1: &str = "asset.import_graph_json_v1";
    pub const IMPORT_DIAGNOSTICS_JSON_V1: &str = "asset.import_diagnostics_json_v1";
    pub const IMPORT_THUMBNAILS_JSON_V1: &str = "asset.import_thumbnails_json_v1";
    pub const IMPORT_DEPENDENCIES_JSON_V1: &str = "asset.import_dependencies_json_v1";
    pub const IMPORT_QUEUE_JSON_V1: &str = "asset.import_queue_json_v1";
    pub const REIMPORT_V1: &str = "asset.reimport_v1";
    pub const THUMBNAIL_JSON_V1: &str = "asset.thumbnail_json_v1";
    pub const DIRTY_SCAN_JSON_V1: &str = "asset.dirty_scan_json_v1";
    pub const PACKAGE_WRITER_INFO_JSON_V1: &str = "asset.package_writer_info_json_v1";

    // Generic lifecycle hook understood by the plugin host.
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

}

pub mod textures_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const MANIFEST_JSON_V1: &str = "assets.textures.manifest_v1";
    pub const ENTRY_RUNTIME_V1: &str = "assets.textures.entry_runtime_v1";
    pub const ENTRY_RGBA8_V1: &str = "assets.textures.entry_rgba8_v1";
    pub const VALIDATE_REF_V1: &str = "assets.textures.validate_ref_v1";
    pub const DESCRIBE_REF_JSON_V1: &str = "assets.textures.describe_ref_json_v1";
}

pub const TEXTURES_SERVICE_METHODS: &[&str] = &[
    textures_method::INFO_JSON,
    textures_method::INVOKE_JSON,
    textures_method::SHUTDOWN_V1,
    textures_method::MANIFEST_JSON_V1,
    textures_method::ENTRY_RUNTIME_V1,
    textures_method::ENTRY_RGBA8_V1,
    textures_method::VALIDATE_REF_V1,
    textures_method::DESCRIBE_REF_JSON_V1,
];

pub mod definitions_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const MANIFEST_JSON_V1: &str = "assets.definitions.manifest_v1";
    pub const ENTRY_JSON_V1: &str = "assets.definitions.entry_v1";
    pub const RESOLVE_REFS_V1: &str = "assets.definitions.resolve_refs_v1";
    pub const VALIDATE_V1: &str = "assets.definitions.validate_v1";
    pub const DESCRIBE_SIDE_EFFECTS_V1: &str = "assets.definitions.describe_side_effects_v1";
}

pub const DEFINITIONS_SERVICE_METHODS: &[&str] = &[
    definitions_method::INFO_JSON,
    definitions_method::INVOKE_JSON,
    definitions_method::SHUTDOWN_V1,
    definitions_method::MANIFEST_JSON_V1,
    definitions_method::ENTRY_JSON_V1,
    definitions_method::RESOLVE_REFS_V1,
    definitions_method::VALIDATE_V1,
    definitions_method::DESCRIBE_SIDE_EFFECTS_V1,
];

pub const TEXTURES_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.textures",
        ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        TEXTURES_SERVICE_ID,
        TEXTURES_BACKEND_CAPABILITY_ID,
    );

pub const DEFINITIONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.definitions",
        ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        DEFINITIONS_SERVICE_ID,
        DEFINITIONS_BACKEND_CAPABILITY_ID,
    );

pub const ASSET_GRAPH_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.graph",
        ENGINE_ASSETS_GRAPH_SERVICE_ID,
        ASSET_GRAPH_SERVICE_ID,
        ASSET_GRAPH_BACKEND_CAPABILITY_ID,
    );

pub const ASSETS_UI_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.ui",
        ENGINE_ASSETS_UI_SERVICE_ID,
        ASSETS_UI_SERVICE_ID,
        ASSETS_UI_BACKEND_CAPABILITY_ID,
    );

pub const TEXTURES_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        TEXTURES_RUNTIME_CONTRACT,
        TEXTURES_SERVICE_METHODS,
    );

pub const TEXTURES_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        TEXTURES_RUNTIME_CONTRACT_SPEC,
        Some(TEXTURES_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_TEXTURES_BACKEND"),
    );

pub const DEFINITIONS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        DEFINITIONS_RUNTIME_CONTRACT,
        DEFINITIONS_SERVICE_METHODS,
    );

pub const DEFINITIONS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        DEFINITIONS_RUNTIME_CONTRACT_SPEC,
        Some(DEFINITIONS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_DEFINITIONS_BACKEND"),
    );

pub const ASSET_GRAPH_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_GRAPH_SERVICE_ID,
        ASSET_GRAPH_RUNTIME_CONTRACT,
        ASSET_GRAPH_SERVICE_METHODS,
    );

pub const ASSET_GRAPH_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ASSET_GRAPH_RUNTIME_CONTRACT_SPEC,
        Some(ASSET_GRAPH_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ASSET_GRAPH_BACKEND"),
    );

pub const ASSETS_UI_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_UI_SERVICE_ID,
        ASSETS_UI_RUNTIME_CONTRACT,
        ASSETS_UI_SERVICE_METHODS,
    );

pub const ASSETS_UI_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ASSETS_UI_RUNTIME_CONTRACT_SPEC,
        Some(ASSETS_UI_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ASSETS_UI_BACKEND"),
    );

/// Required runtime methods for AssetManager 0.6+ deployments.
///
/// The engine validates these before scene bootstrap so an old DLL cannot fail
/// later as "unknown method" inside foliage/profile loading.
pub const REQUIRED_RUNTIME_METHODS_V1: &[&str] = &[
    method::INFO_JSON,
    method::INVOKE_JSON,
    method::SHUTDOWN_V1,
    method::RAW_BYTES_V1,
    method::TEXT_V1,
    method::IMPORT_V1,
    method::TEXTURE_RGBA8_V1,
    method::DECODE_V1,
    method::PUMP_V1,
    method::STATUS_JSON_V1,
    method::STATUS_GRAPH_JSON_V1,
    method::PROJECT_STATUS_JSON_V1,
    method::FORMATS_JSON_V1,
    method::SOURCES_JSON_V1,
    method::VFS_LIST_JSON_V1,
    method::LIST_FILE_REPACK_JSON_V1,
    method::UID_JSON_V1,
    method::IMPORT_CACHE_JSON_V1,
    method::IMPORT_DIRTY_JSON_V1,
    method::IMPORT_SCAN_JSON_V1,
    method::IMPORT_GRAPH_JSON_V1,
    method::IMPORT_DIAGNOSTICS_JSON_V1,
    method::IMPORT_THUMBNAILS_JSON_V1,
    method::IMPORT_DEPENDENCIES_JSON_V1,
    method::IMPORT_QUEUE_JSON_V1,
    method::REIMPORT_V1,
    method::THUMBNAIL_JSON_V1,
    method::DIRTY_SCAN_JSON_V1,
    method::PACKAGE_WRITER_INFO_JSON_V1,
];

/// Startup validation contract for the engine-facing asset gateway.
///
/// Validation reads the active backend provider description through the gateway.
pub const ASSET_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ASSET_SERVICE_ID,
        "newengine.assets-api >= 0.8.x",
        REQUIRED_RUNTIME_METHODS_V1,
    );

/// Declarative startup requirement for the engine-facing asset gateway. Missing
/// assets degrade unless a strict runtime profile explicitly requires them.
pub const ASSET_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ASSET_RUNTIME_CONTRACT_SPEC,
        Some(ASSET_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ASSET_MANAGER"),
    );

/// Runtime-ready texture packet returned by AssetManager.
///
/// Important: this is not a decoder contract. The codec pipeline must already
/// have converted the source container (DDS/PNG/JPEG/etc.) into RGBA8 or an
/// explicit renderer-native payload. Runtime code only consumes this normalized
/// packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8TextureAsset {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Rgba8TextureAsset {
    #[inline]
    pub fn expected_len(width: u32, height: u32) -> usize {
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    }

    #[inline]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err(format!("rgba8 texture has zero extent {width}x{height}"));
        }
        let expected = Self::expected_len(width, height);
        if rgba.len() != expected {
            return Err(format!(
                "rgba8 texture payload size mismatch bytes={} expected={} extent={}x{}",
                rgba.len(), expected, width, height
            ));
        }
        Ok(Self { width, height, rgba })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTextureFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Bc1RgbaUnorm,
    Bc1RgbaSrgb,
    Bc3RgbaUnorm,
    Bc3RgbaSrgb,
    Bc5RgUnorm,
    Bc7RgbaUnorm,
    Bc7RgbaSrgb,
}

impl RuntimeTextureFormat {
    #[inline]
    pub const fn as_wire_id(self) -> u16 {
        match self {
            Self::Rgba8Unorm => 1,
            Self::Rgba8Srgb => 2,
            Self::Bc1RgbaUnorm => 101,
            Self::Bc1RgbaSrgb => 102,
            Self::Bc3RgbaUnorm => 103,
            Self::Bc3RgbaSrgb => 104,
            Self::Bc5RgUnorm => 105,
            Self::Bc7RgbaUnorm => 106,
            Self::Bc7RgbaSrgb => 107,
        }
    }

    #[inline]
    pub const fn from_wire_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::Rgba8Unorm),
            2 => Some(Self::Rgba8Srgb),
            101 => Some(Self::Bc1RgbaUnorm),
            102 => Some(Self::Bc1RgbaSrgb),
            103 => Some(Self::Bc3RgbaUnorm),
            104 => Some(Self::Bc3RgbaSrgb),
            105 => Some(Self::Bc5RgUnorm),
            106 => Some(Self::Bc7RgbaUnorm),
            107 => Some(Self::Bc7RgbaSrgb),
            _ => None,
        }
    }

    #[inline]
    pub fn from_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "RGBA8_UNORM" | "RGBA8" => Some(Self::Rgba8Unorm),
            "RGBA8_SRGB" => Some(Self::Rgba8Srgb),
            "BC1_RGBA_UNORM" | "BC1_UNORM" | "BC1" => Some(Self::Bc1RgbaUnorm),
            "BC1_RGBA_SRGB" | "BC1_SRGB" => Some(Self::Bc1RgbaSrgb),
            "BC3_RGBA_UNORM" | "BC3_UNORM" | "BC3" => Some(Self::Bc3RgbaUnorm),
            "BC3_RGBA_SRGB" | "BC3_SRGB" => Some(Self::Bc3RgbaSrgb),
            "BC5_RG_UNORM" | "BC5_UNORM" | "BC5" => Some(Self::Bc5RgUnorm),
            "BC7_RGBA_UNORM" | "BC7_UNORM" | "BC7" => Some(Self::Bc7RgbaUnorm),
            "BC7_RGBA_SRGB" | "BC7_SRGB" => Some(Self::Bc7RgbaSrgb),
            _ => None,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8Unorm => "RGBA8_UNORM",
            Self::Rgba8Srgb => "RGBA8_SRGB",
            Self::Bc1RgbaUnorm => "BC1_RGBA_UNORM",
            Self::Bc1RgbaSrgb => "BC1_RGBA_SRGB",
            Self::Bc3RgbaUnorm => "BC3_RGBA_UNORM",
            Self::Bc3RgbaSrgb => "BC3_RGBA_SRGB",
            Self::Bc5RgUnorm => "BC5_RG_UNORM",
            Self::Bc7RgbaUnorm => "BC7_RGBA_UNORM",
            Self::Bc7RgbaSrgb => "BC7_RGBA_SRGB",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextureMip {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextureAsset {
    pub width: u32,
    pub height: u32,
    pub format: RuntimeTextureFormat,
    pub mips: Vec<RuntimeTextureMip>,
}

impl RuntimeTextureAsset {
    #[inline]
    pub fn concatenated_payload_and_layout(&self) -> (Vec<u8>, Vec<RuntimeTextureMipLayout>) {
        let mut data = Vec::new();
        let mut layout = Vec::with_capacity(self.mips.len());
        for mip in &self.mips {
            let offset = data.len() as u64;
            data.extend_from_slice(&mip.bytes);
            layout.push(RuntimeTextureMipLayout {
                level: mip.level,
                width: mip.width,
                height: mip.height,
                offset,
                byte_len: mip.bytes.len() as u64,
            });
        }
        (data, layout)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTextureMipLayout {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub offset: u64,
    pub byte_len: u64,
}

pub mod texture_wire {
    pub const MAGIC: [u8; 4] = *b"NTRT";
    pub const VERSION_RGBA8_V1: u16 = 1;
    pub const VERSION_RUNTIME_V2: u16 = 2;
    pub const HEADER_LEN: usize = 20;
    pub const RUNTIME_HEADER_LEN: usize = 32;
    pub const RUNTIME_MIP_RECORD_LEN: usize = 20;
}

/// Asset lifecycle state as observed through an AssetManager-like service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Unloaded,
    Loading,
    Ready,
    Failed,
    Unknown,
}

/// Residency domain. Stages are meaningful only inside a domain: VFS bytes, CPU-decoded payloads, and GPU resources are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetResidencyDomain {
    Vfs,
    Cpu,
    Gpu,
    Unknown,
}

impl AssetResidencyDomain {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vfs => "vfs",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Unknown => "unknown",
        }
    }
}

impl core::fmt::Display for AssetResidencyDomain {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable, high-resolution asset lifecycle stage used by tooling, loading screens and render gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetStatusStage {
    Declared,
    Requested,
    Resolving,
    Queued,
    Reading,
    Importing,
    Imported,
    UploadQueued,
    Uploading,
    Resident,
    Failed,
    Stale,
    Unknown,
}

impl AssetStatusStage {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Requested => "requested",
            Self::Resolving => "resolving",
            Self::Queued => "queued",
            Self::Reading => "reading",
            Self::Importing => "importing",
            Self::Imported => "imported",
            Self::UploadQueued => "upload_queued",
            Self::Uploading => "uploading",
            Self::Resident => "resident",
            Self::Failed => "failed",
            Self::Stale => "stale",
            Self::Unknown => "unknown",
        }
    }
}

impl core::fmt::Display for AssetStatusStage {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// AssetStatus is the canonical read-model row for one asset graph node.
///
/// The service serializes the same shape as JSON via `asset.status_json_v1`.
/// Runtime systems may keep richer local states, but they should be projected
/// from this model instead of inventing incompatible lifecycle enums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetStatus {
    pub id_hex32: String,
    pub logical_path: Option<String>,
    pub state: AssetState,
    pub domain: AssetResidencyDomain,
    pub stage: AssetStatusStage,
    pub source: Option<String>,
    pub codec_id: Option<String>,
    pub type_id: Option<String>,
    pub format: Option<String>,
    pub bytes: Option<u64>,
    pub error: Option<String>,
    pub detail: Option<String>,
    pub updated_unix_ms: u64,
}

impl AssetStatus {
    #[inline]
    pub fn unknown(id_hex32: impl Into<String>) -> Self {
        Self {
            id_hex32: id_hex32.into(),
            logical_path: None,
            state: AssetState::Unknown,
            domain: AssetResidencyDomain::Unknown,
            stage: AssetStatusStage::Unknown,
            source: None,
            codec_id: None,
            type_id: None,
            format: None,
            bytes: None,
            error: None,
            detail: Some("AssetManager has no status row for this asset".to_string()),
            updated_unix_ms: 0,
        }
    }
}

/// Minimal engine-facing Asset access surface.
///
/// Implementations may be plugin-backed, filesystem-backed, HTTP-backed, etc.
pub trait AssetAccess {
    /// Enqueue codec-owned asset load/decode by logical path. Returns an opaque stable id (hex32 string).
    fn import_v1(&self, logical_path: &str) -> Result<String, String>;

    /// Progress background AssetManager work through the stable v1 pump method.
    fn pump(&self);

    /// Query current state for an enqueued asset.
    fn state(&self, id_hex32: &str) -> Result<AssetState, String>;

    /// Query the canonical AssetStatus read-model row for an asset id or logical path.
    fn status_json_v1(&self, id_or_logical_path: &str) -> Result<serde_json::Value, String>;

    /// Query the full AssetStatus graph node for an asset id or logical path.
    fn status_graph_json_v1(&self, id_or_logical_path: &str) -> Result<serde_json::Value, String>;

    /// Project a validated lifecycle transition from an owning subsystem, e.g. render GPU residency.
    fn project_status_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Read UTF-8/text asset bytes by logical path through AssetManager/VFS.
    fn text_v1(&self, logical_path: &str) -> Result<Vec<u8>, String>;

    /// Read raw binary asset bytes by logical path through AssetManager/VFS.
    ///
    /// This is still AssetManager-owned VFS access; callers must not use
    /// filesystem paths or bypass mounts.
    fn raw_bytes_v1(&self, logical_path: &str) -> Result<Vec<u8>, String>;

    /// Read asset payload using a stable wire format.
    ///
    /// Returns `(meta_json, payload_bytes)`.
    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String>;

    /// Decode any registered runtime asset container through the codec registry.
    fn decode_v1(&self, request: &AssetDecodeRequest) -> Result<Vec<u8>, String>;

    /// Read a runtime-ready RGBA8 texture packet.
    ///
    /// The implementation must parse/validate codec metadata inside AssetManager.
    /// Runtime callers must not parse image containers or codec metadata.
    fn texture_rgba8_v1(&self, id_hex32: &str) -> Result<Rgba8TextureAsset, String>;

    /// Select and read a runtime-ready RGBA8 texture from a .ytd dictionary.
    fn texture_dictionary_rgba8_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<Rgba8TextureAsset, String>;

    /// Select and read a runtime-ready GPU-native texture from a .ytd dictionary.
    fn texture_dictionary_runtime_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<RuntimeTextureAsset, String>;

    /// Select and read a runtime-ready RGBA8 texture through semantic `engine.assets.textures` ownership.
    ///
    /// Generic AssetAccess implementors may bridge to the older dictionary methods, but the
    /// canonical runtime host implementation routes this through `engine.assets.assets.textures.entry_rgba8_v1`.
    fn textures_entry_rgba8_v1(&self, texture_ref: &str) -> Result<Rgba8TextureAsset, String> {
        let reference = require_asset_reference_extension(texture_ref, &["ytd"], true)
            .map_err(|e| e.to_string())?;
        let entry = reference.entry.as_deref().unwrap_or_default();
        let texture_hash = entry
            .strip_prefix("hash:")
            .map(|value| value.parse::<u64>().map_err(|_| format!("invalid texture hash selector '{entry}'")))
            .transpose()?;
        let texture_name = if texture_hash.is_some() { None } else { Some(entry) };
        self.texture_dictionary_rgba8_v1(&reference.logical_path, texture_name, texture_hash)
    }

    /// Select and read a runtime-ready GPU-native texture through semantic `engine.assets.textures` ownership.
    ///
    /// Generic AssetAccess implementors may bridge to the older dictionary methods, but the
    /// canonical runtime host implementation routes this through `engine.assets.assets.textures.entry_runtime_v1`.
    fn textures_entry_runtime_v1(&self, texture_ref: &str) -> Result<RuntimeTextureAsset, String> {
        let reference = require_asset_reference_extension(texture_ref, &["ytd"], true)
            .map_err(|e| e.to_string())?;
        let entry = reference.entry.as_deref().unwrap_or_default();
        let texture_hash = entry
            .strip_prefix("hash:")
            .map(|value| value.parse::<u64>().map_err(|_| format!("invalid texture hash selector '{entry}'")))
            .transpose()?;
        let texture_name = if texture_hash.is_some() { None } else { Some(entry) };
        self.texture_dictionary_runtime_v1(&reference.logical_path, texture_name, texture_hash)
    }
}

/// Extended contract surface.
///
/// Keep this trait small and data-oriented; higher-level systems can build their own
/// caches and decoders above these primitives.
pub trait AssetService: AssetAccess {
    /// Reload/reimport asset by logical path through the stable v1 reload method.
    fn reload(&self, logical_path: &str) -> Result<String, String>;

    /// Query extended info by logical path.
    fn info_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String>;

    /// List known formats.
    fn formats_json_v1(&self) -> Result<serde_json::Value, String>;

    /// List mounted sources.
    fn sources_json_v1(&self) -> Result<serde_json::Value, String>;

    /// List a mounted VFS directory through AssetManager, not through direct filesystem paths.
    fn vfs_list_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String>;

    /// Rebuild/repack a NEF8 ListFile and write it back through the winning writable VFS source.
    fn list_file_repack_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Return the engine.assets UID row for a logical asset.
    fn uid_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String>;

    /// Return the editor/import cache projection over status, codec metadata and dirty flags.
    fn import_cache_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Mark one logical asset dirty/stale; file watchers should use this before reload/reimport.
    fn import_dirty_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Bounded VFS scan for editor/import discovery.
    fn import_scan_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Import dependency graph projection for one asset.
    fn import_graph_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Human-readable import diagnostics.
    fn import_diagnostics_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Editor thumbnail metadata/cache-key plan. Final pixels belong to render/UI providers.
    fn import_thumbnails_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Direct dependency/dependent list for one asset.
    fn import_dependencies_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Queue read-model for background import work.
    fn import_queue_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Explicit dirty+reload lifecycle command.
    fn reimport_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Single asset thumbnail metadata/cache-key plan.
    fn thumbnail_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Bounded scan that classifies missing/dirty/stale rows for editor reimport.
    fn dirty_scan_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Package/listFile writer capability diagnostics.
    fn package_writer_info_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Mount one source through the strict v1 JSON source model.
    fn mount_source_json_v1(&self, payload: serde_json::Value) -> Result<(), String>;

    /// Returns a deterministic trace describing which sources contain the asset.
    fn resolve_trace_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String>;

}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitReadyError {
    Timeout,
    Failed(String),
    Transport(String),
}

/// Progress the asset lifecycle once and return readiness.
///
/// This function deliberately does not sleep or spin. Runtime/editor callers must
/// call it from a frame, job, or asset-event callback and retry after the asset
/// pipeline publishes more work. `timeout` is retained as a compatibility guard
/// for callers that pass an already-expired deadline budget.
pub fn wait_ready<A: AssetAccess + ?Sized>(
    assets: &A,
    id_hex32: &str,
    timeout: Duration,
) -> Result<(), WaitReadyError> {
    if timeout.is_zero() {
        return Err(WaitReadyError::Timeout);
    }

    assets.pump();

    match assets.state(id_hex32) {
        Ok(AssetState::Ready) => Ok(()),
        Ok(AssetState::Failed) => Err(WaitReadyError::Failed(id_hex32.to_string())),
        Ok(AssetState::Loading) | Ok(AssetState::Unloaded) => Err(WaitReadyError::Timeout),
        Ok(AssetState::Unknown) => {
            log::warn!("Unknown asset state id='{}'", id_hex32);
            Err(WaitReadyError::Timeout)
        }
        Err(e) => Err(WaitReadyError::Transport(e)),
    }
}

#[cfg(test)]
mod file_type_layer_contract_tests {
    use super::*;

    #[test]
    fn descriptor_normalization_does_not_infer_semantic_owner() {
        let mut descriptor = AssetFileTypeDescriptor {
            extension: "whatever".to_owned(),
            asset_kind: "opaque_format".to_owned(),
            codec_type: codec_type::LIST_FILE.to_owned(),
            handler_service: "asset.codec.listfile.whatever".to_owned(),
            magic: Some("4e454638".to_owned()),
            ..Default::default()
        };
        descriptor.normalize_layer_contract();
        assert!(descriptor.semantic_gateway.is_empty());
        assert!(descriptor.validate_generic_rules().is_err());
    }

    #[test]
    fn explicit_descriptor_is_valid_without_registry_extension_knowledge() {
        let mut descriptor = AssetFileTypeDescriptor {
            extension: "opaque".to_owned(),
            asset_kind: "provider_declared_asset".to_owned(),
            container: "newengine.listfile.nef8.opaque".to_owned(),
            content_kind: Some(9001),
            codec_type: codec_type::LIST_FILE.to_owned(),
            byte_owner: ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: "engine.assets.provider_declared".to_owned(),
            handler_service: "asset.codec.listfile.opaque".to_owned(),
            selector_syntax: Some("file.opaque@entry".to_owned()),
            consumer_domains: vec!["engine.assets.provider_declared".to_owned()],
            magic: Some("4e454638".to_owned()),
            outputs: vec![ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(), ASSET_LIST_FILE_BODY_OUTPUT.to_owned()],
            runtime_ready: true,
            native_container: true,
            requires_magic: true,
            ..Default::default()
        };
        descriptor.normalize_layer_contract();
        assert_eq!(descriptor.gateway, descriptor.semantic_gateway);
        assert_eq!(descriptor.content_kind, Some(9001));
        assert!(descriptor.validate_generic_rules().is_ok());
    }
}
