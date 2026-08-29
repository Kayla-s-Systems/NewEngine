#![forbid(unsafe_op_in_unsafe_fn)]

mod raw_range;
pub use raw_range::*;
mod asset_error;
pub use asset_error::*;

mod asset_service_client;
pub use asset_service_client::AssetServiceClient;

mod asset_document;
pub use asset_document::*;

mod pipeline;
pub use pipeline::*;

mod file_types;
pub use file_types::*;

mod texture_assets;
pub use texture_assets::*;

mod map_assets;
pub use map_assets::*;

mod asset_lifecycle;
pub use asset_lifecycle::*;

mod asset_streaming;
pub use asset_streaming::*;

mod source_dictionary;
pub use source_dictionary::*;

/// Engine-facing asset service gateway id.
///
/// Runtime and provider plugins must request assets through this stable host-owned
/// gateway, not through a concrete AssetManager/provider service id. The host
/// resolves this gateway to the active asset backend by declared capability.
pub const ENGINE_ASSET_SERVICE_ID: &str = newengine_service_api::ENGINE_ASSETS_GATEWAY_ID;

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

/// Canonical provider-neutral VFS selection rule. Providers may implement the
/// storage and codecs differently, but a compiled candidate for one logical
/// asset id must be considered before its authoring-source candidate.
pub const ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1: &str =
    "compiled_first_source_fallback.v1";

/// Stable JSON values accepted by asset.mount_source_json_v1 in asset_role.
pub mod asset_source_role {
    pub const COMPILED: &str = "compiled";
    pub const RUNTIME: &str = "runtime";
    pub const SOURCE: &str = "source";
}

/// Semantic texture dictionary runtime gateway id. File-type descriptors route `.ytd`
/// meaning here. `engine.assets.textures` owns validation, manifest semantics and runtime
/// texture packets; `engine.assets` remains the byte/VFS/codec-dispatch owner.
pub const ENGINE_ASSETS_TEXTURES_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_TEXTURES_GATEWAY_ID;
pub const TEXTURES_SERVICE_ID: &str = "textures.api";
pub const TEXTURES_BACKEND_CAPABILITY_ID: &str = "assets.textures.backend";
pub const TEXTURES_RUNTIME_CONTRACT: &str = "newengine.assets.textures.runtime.v1";
/// Semantic definition/archetype metadata gateway id. File-type descriptors route
/// `.ytyp` meaning here; scene/world systems may consume definitions later, but
/// do not own the file type.
pub const ENGINE_ASSETS_DEFINITIONS_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_DEFINITIONS_GATEWAY_ID;
pub const DEFINITIONS_SERVICE_ID: &str = "definitions.api";
pub const DEFINITIONS_BACKEND_CAPABILITY_ID: &str = "assets.definitions.backend";
pub const DEFINITIONS_RUNTIME_CONTRACT: &str = "newengine.assets.definitions.runtime.v1";
pub const ENGINE_ASSETS_MODELS_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_MODELS_GATEWAY_ID;
pub const ENGINE_ASSETS_MATERIALS_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_MATERIALS_GATEWAY_ID;
/// Semantic authored map/world placement gateway id. `.ymap` owns map composition,
/// placements and references to `.ytyp` Definition Entries; it does not replace
/// `.ytyp` as the generic metadata/knowledge source.
pub const ENGINE_ASSETS_MAPS_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_MAPS_GATEWAY_ID;
pub const MAPS_SERVICE_ID: &str = "maps.api";
pub const MAPS_BACKEND_CAPABILITY_ID: &str = "assets.maps.backend";
pub const MAPS_RUNTIME_CONTRACT: &str = "newengine.assets.maps.runtime.v1";

/// Semantic UI dictionary gateway id. `.neui` meaning lives here: XMLcentral validation,
/// entry selection, binding/action/dependency extraction and authored-to-runtime DTO
/// compilation. `engine.assets` remains byte/VFS/codec owner; `engine.ui` remains live runtime.
pub const ENGINE_ASSETS_UI_SERVICE_ID: &str = newengine_service_api::ENGINE_ASSETS_UI_GATEWAY_ID;
pub const ASSETS_UI_SERVICE_ID: &str = "assets.ui.api";
pub const ASSETS_UI_BACKEND_CAPABILITY_ID: &str = "assets.ui.backend";
pub const ASSETS_UI_RUNTIME_CONTRACT: &str = "newengine.assets.ui.runtime.v1";

/// Runtime scene gateway. It consumes resolved map/definition DTOs and mutates the world; it does not own authored map file semantics.
pub const ENGINE_SCENE_SERVICE_ID: &str = newengine_service_api::ENGINE_SCENE_GATEWAY_ID;

/// Runtime scripting gateway. `.ysc` script modules are opaque to core and
/// are routed through this domain; AssetManager still owns VFS bytes and ListFile codec dispatch.
pub const ENGINE_SCRIPTING_SERVICE_ID: &str = newengine_service_api::ENGINE_SCRIPTING_GATEWAY_ID;

/// Semantic asset graph gateway id. This resolver owns declarative dependency
/// graph expansion over .ytyp/.ydd/.nemat/.ytd refs; it uses engine.assets only
/// for VFS bytes and codec dispatch.
pub const ENGINE_ASSETS_GRAPH_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_GRAPH_GATEWAY_ID;
pub const ASSET_GRAPH_SERVICE_ID: &str = "asset_graph.api";
pub const ASSET_GRAPH_BACKEND_CAPABILITY_ID: &str = "assets.graph.backend";
pub const ASSET_GRAPH_RUNTIME_CONTRACT: &str = "newengine.assets.graph.runtime.v1";

/// Editor/import lifecycle sub-gateways and capability ids.
///
/// These are Godot-inspired lifecycle surfaces, but they do not adopt Godot's
/// `.tres/.res` resource model. `engine.assets` remains the byte/VFS/codec host;
/// these sub-gateways expose inspectable editor/import read-model slices.
pub const ENGINE_ASSETS_UID_SERVICE_ID: &str = newengine_service_api::ENGINE_ASSETS_UID_GATEWAY_ID;
pub const ASSETS_UID_SERVICE_ID: &str = "assets.uid.api";
pub const ASSETS_UID_BACKEND_CAPABILITY_ID: &str = "assets.uid.backend";
pub const ASSETS_UID_RUNTIME_CONTRACT: &str = "newengine.assets.uid.v1";

pub const ENGINE_ASSETS_DEPENDENCIES_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_DEPENDENCIES_GATEWAY_ID;
pub const ASSETS_DEPENDENCIES_SERVICE_ID: &str = "assets.dependencies.api";
pub const ASSETS_DEPENDENCIES_BACKEND_CAPABILITY_ID: &str = "assets.dependencies.backend";
pub const ASSETS_DEPENDENCIES_RUNTIME_CONTRACT: &str = "newengine.assets.dependencies.v1";

pub const ENGINE_ASSETS_IMPORT_QUEUE_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_IMPORT_QUEUE_GATEWAY_ID;
pub const ASSETS_IMPORT_QUEUE_SERVICE_ID: &str = "assets.import_queue.api";
pub const ASSETS_IMPORT_QUEUE_BACKEND_CAPABILITY_ID: &str = "assets.import_queue.backend";
pub const ASSETS_IMPORT_QUEUE_RUNTIME_CONTRACT: &str = "newengine.assets.import_queue.v1";

pub const ENGINE_ASSETS_PACKAGE_WRITER_SERVICE_ID: &str =
    newengine_service_api::ENGINE_ASSETS_PACKAGE_WRITER_GATEWAY_ID;
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
// `engine.assets.streaming` is currently a child gateway routed to the same
// `asset_manager.api` provider service as the root `engine.assets` gateway. Runtime
// contract validation therefore checks the provider service family here; the
// streaming-specific surface is enforced by `ASSETS_STREAMING_SERVICE_METHODS` and
// `assets.streaming.backend`. A dedicated future streaming provider may publish its
// own exact runtime contract family once it owns a distinct provider service.
pub const ASSETS_STREAMING_RUNTIME_CONTRACT: &str = "newengine.assets-api >= 0.8.x";

/// Concrete provider routing for the global asset residency controller.
/// The provider service remains `asset_manager.api`; this sub-gateway is the
/// ownership boundary for request/pin/residency/cleanup policy.
pub const ASSETS_STREAMING_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.streaming",
        ENGINE_ASSETS_STREAMING_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSETS_STREAMING_BACKEND_CAPABILITY_ID,
    );

/// World streaming gateway and capability ids.
///
/// World streaming owns cell visibility and spatial budget decisions. It may
/// request asset streaming work, but all loader/visibility jobs must remain on
/// the engine job system and visible in diagnostics/profiler.
pub const ENGINE_WORLD_STREAMING_SERVICE_ID: &str = "engine.world.streaming";
pub const WORLD_STREAMING_SERVICE_ID: &str = "world.streaming.api";
pub const WORLD_STREAMING_BACKEND_CAPABILITY_ID: &str = "world.streaming.backend";
pub const WORLD_STREAMING_CELLS_CAPABILITY_ID: &str = "world.streaming.cells";
pub const WORLD_STREAMING_VISIBILITY_BUDGET_CAPABILITY_ID: &str =
    "world.streaming.visibility_budget";
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
    pub const INSPECT_DIALECT_V1: &str = "assets.ui.inspect_dialect_v1";
    pub const INVALIDATE_V1: &str = "assets.ui.invalidate_v1";
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
    assets_ui_method::INSPECT_DIALECT_V1,
    assets_ui_method::INVALIDATE_V1,
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
pub const ENGINE_ASSET_TYPES_SERVICE_ID: &str = "engine.assets.types";
pub const ASSET_TYPES_SERVICE_ID: &str = "asset.types.api";
pub const ASSET_TYPES_BACKEND_CAPABILITY_ID: &str = "assets.types.backend";

pub const ASSET_TYPES_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.types",
        ENGINE_ASSET_TYPES_SERVICE_ID,
        ASSET_TYPES_SERVICE_ID,
        ASSET_TYPES_BACKEND_CAPABILITY_ID,
    );

pub mod file_type_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const MANIFEST_JSON_V1: &str = "asset.types.manifest_json_v1";
    pub const REGISTER_JSON_V1: &str = "asset.types.register_json_v1";
    pub const PROBE_JSON_V1: &str = "asset.types.probe_json_v1";
    pub const RESOLVE_JSON_V1: &str = "asset.types.resolve_json_v1";
}

pub const ASSET_TYPES_SERVICE_METHODS: &[&str] = &[
    file_type_method::INFO_JSON,
    file_type_method::INVOKE_JSON,
    file_type_method::SHUTDOWN_V1,
    file_type_method::MANIFEST_JSON_V1,
    file_type_method::REGISTER_JSON_V1,
    file_type_method::PROBE_JSON_V1,
    file_type_method::RESOLVE_JSON_V1,
];

pub const ASSET_TYPES_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSET_TYPES_SERVICE_ID,
        "newengine.assets-types-api >= 0.1.x",
        ASSET_TYPES_SERVICE_METHODS,
    );

pub const ASSET_TYPES_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ASSET_TYPES_RUNTIME_CONTRACT_SPEC,
        Some(ASSET_TYPES_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ASSET_TYPES"),
    );

/// Asset codec classification used by AssetManager to apply generic host rules.
///
/// The host does not interpret concrete formats. It only enforces broad safety
/// constraints declared by the codec provider. Encoding/compression is not part
/// of this enum; codecs own their internal source envelope and may support
/// raw XML, native binary, deflate, etc. behind the same logical descriptor.
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
    /// Bounded raw VFS byte range. Request is JSON `AssetRawRangeRequest`; response is NARR v1 binary.
    pub const RAW_RANGE_V1: &str = "asset.raw_range_v1";
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
    /// Mount payload accepts asset_role and aliases [{ logical_path, source_path }].
    /// All compiled mounts precede source mounts regardless of numeric priority.
    pub const MOUNT_SOURCE_JSON_V1: &str = "asset.mount_source_json_v1";

    // Debug/diagnostics.
    pub const RESOLVE_TRACE_JSON_V1: &str = "asset.resolve_trace_json_v1";
    /// Standard listFiles manifest for any dictionary/container asset. Codec-defined output; not a raw VFS read.
    pub const LIST_FILE_MANIFEST: &str = "asset.list_file_manifest";

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
    /// Full provider-neutral runtime dependency graph used by hot-reload/invalidation planners.
    pub const RUNTIME_GRAPH_JSON_V1: &str = "asset.runtime_graph_json_v1";
    pub const IMPORT_DIAGNOSTICS_JSON_V1: &str = "asset.import_diagnostics_json_v1";
    pub const IMPORT_THUMBNAILS_JSON_V1: &str = "asset.import_thumbnails_json_v1";
    pub const IMPORT_DEPENDENCIES_JSON_V1: &str = "asset.import_dependencies_json_v1";
    pub const IMPORT_QUEUE_JSON_V1: &str = "asset.import_queue_json_v1";
    pub const REIMPORT_V1: &str = "asset.reimport_v1";
    pub const THUMBNAIL_JSON_V1: &str = "asset.thumbnail_json_v1";
    pub const DIRTY_SCAN_JSON_V1: &str = "asset.dirty_scan_json_v1";
    pub const PACKAGE_WRITER_INFO_JSON_V1: &str = "asset.package_writer_info_json_v1";
    /// Explicit .nepak package writer execution. Payload is NepakPackageWriteRequestV1.
    pub const PACKAGE_WRITE_NEPAK_JSON_V1: &str = "asset.package_write_nepak_json_v1";
    /// Explicit UTF-8 text replacement through the winning writable VFS source.
    pub const PACKAGE_WRITE_TEXT_JSON_V1: &str = "asset.package_write_text_json_v1";

    // Global runtime residency / memory controller.
    pub const STREAMING_REQUEST_V1: &str = "asset.streaming.request_v1";
    /// Scheduler-selected provider admission. Demand selection remains engine-owned.
    pub const STREAMING_ADMIT_V2: &str = "asset.streaming.admit_v2";
    /// Exact scheduler-selected CPU residency eviction.
    pub const STREAMING_EVICT_V2: &str = "asset.streaming.evict_v2";
    /// Provider lifecycle acknowledgement used to reconcile engine residency state.
    pub const STREAMING_LIFECYCLE_V2: &str = "asset.streaming.lifecycle_v2";
    pub const STREAMING_PIN_V1: &str = "asset.streaming.pin_v1";
    pub const STREAMING_UNPIN_V1: &str = "asset.streaming.unpin_v1";
    pub const STREAMING_TOUCH_V1: &str = "asset.streaming.touch_v1";
    pub const STREAMING_CLEANUP_V1: &str = "asset.streaming.cleanup_v1";
    pub const STREAMING_COMPACT_V1: &str = "asset.streaming.compact_v1";
    pub const STREAMING_STATS_V1: &str = "asset.streaming.stats_v1";

    // Generic lifecycle hook understood by the plugin host.
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
}

/// Normative contract behind the generic `asset.decode_v1` codec boundary.
/// The advertised id intentionally remains the method token already carried by
/// AssetManager/codec descriptors; the stable registry key is version-neutral.
pub const ASSET_DECODE_PROTOCOL_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "asset.decode.protocol",
        newengine_contract_api::ContractKind::Protocol,
        newengine_contract_api::ContractVersion::major(1),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-assets-api",
        Some(method::DECODE_V1),
    );

/// Normative contract for editor/package write-back through container codecs.
pub const CONTAINER_WRITE_BYTES_PROTOCOL_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "asset.container.write_bytes.protocol",
        newengine_contract_api::ContractKind::Protocol,
        newengine_contract_api::ContractVersion::major(1),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-assets-api",
        Some("container.write_bytes_v1"),
    );

/// Descriptor-driven preview contract shared by AssetManager and Editor surfaces.
pub const ASSET_PREVIEW_PROTOCOL_ID: &str = "newengine.assets.preview.contract.v1";
pub const ASSET_PREVIEW_PROTOCOL_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "asset.preview.protocol",
        newengine_contract_api::ContractKind::Protocol,
        newengine_contract_api::ContractVersion::major(1),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-assets-api",
        Some(ASSET_PREVIEW_PROTOCOL_ID),
    );

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

pub mod maps_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const INDEX_V1: &str = "assets.maps.index_v1";
    pub const CELL_V1: &str = "assets.maps.cell_v1";
    pub const CELL_V2: &str = "assets.maps.cell_v2";
    pub const VALIDATE_V1: &str = "assets.maps.validate_v1";
    pub const DEPENDENCIES_V1: &str = "assets.maps.dependencies_v1";
}

pub const MAPS_SERVICE_METHODS: &[&str] = &[
    maps_method::INFO_JSON,
    maps_method::INVOKE_JSON,
    maps_method::SHUTDOWN_V1,
    maps_method::INDEX_V1,
    maps_method::CELL_V1,
    maps_method::CELL_V2,
    maps_method::VALIDATE_V1,
    maps_method::DEPENDENCIES_V1,
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

pub const MAPS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.maps",
        ENGINE_ASSETS_MAPS_SERVICE_ID,
        MAPS_SERVICE_ID,
        MAPS_BACKEND_CAPABILITY_ID,
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

pub const DEFINITIONS_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        DEFINITIONS_RUNTIME_CONTRACT_SPEC,
        Some(DEFINITIONS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_DEFINITIONS_BACKEND"),
    );

pub const MAPS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_MAPS_SERVICE_ID,
        MAPS_RUNTIME_CONTRACT,
        MAPS_SERVICE_METHODS,
    );

pub const MAPS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        MAPS_RUNTIME_CONTRACT_SPEC,
        Some(MAPS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_MAPS_BACKEND"),
    );

pub const ASSET_GRAPH_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_GRAPH_SERVICE_ID,
        ASSET_GRAPH_RUNTIME_CONTRACT,
        ASSET_GRAPH_SERVICE_METHODS,
    );

pub const ASSET_GRAPH_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
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

pub const ASSETS_STREAMING_SERVICE_METHODS: &[&str] = &[
    newengine_service_api::SERVICE_METHOD_INFO_JSON,
    newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
    method::STREAMING_REQUEST_V1,
    method::STREAMING_ADMIT_V2,
    method::STREAMING_EVICT_V2,
    method::STREAMING_LIFECYCLE_V2,
    method::STREAMING_PIN_V1,
    method::STREAMING_UNPIN_V1,
    method::STREAMING_TOUCH_V1,
    method::STREAMING_CLEANUP_V1,
    method::STREAMING_COMPACT_V1,
    method::STREAMING_STATS_V1,
];

pub const ASSETS_STREAMING_RUNTIME_CONTRACT_SPEC:
    newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_STREAMING_SERVICE_ID,
        ASSETS_STREAMING_RUNTIME_CONTRACT,
        ASSETS_STREAMING_SERVICE_METHODS,
    );

pub const ASSETS_STREAMING_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ASSETS_STREAMING_RUNTIME_CONTRACT_SPEC,
        Some(ASSETS_STREAMING_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ASSET_STREAMING_BACKEND"),
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
    method::RAW_RANGE_V1,
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
    method::RUNTIME_GRAPH_JSON_V1,
    method::IMPORT_DIAGNOSTICS_JSON_V1,
    method::IMPORT_THUMBNAILS_JSON_V1,
    method::IMPORT_DEPENDENCIES_JSON_V1,
    method::IMPORT_QUEUE_JSON_V1,
    method::REIMPORT_V1,
    method::THUMBNAIL_JSON_V1,
    method::DIRTY_SCAN_JSON_V1,
    method::PACKAGE_WRITER_INFO_JSON_V1,
    method::PACKAGE_WRITE_NEPAK_JSON_V1,
    method::PACKAGE_WRITE_TEXT_JSON_V1,
    method::STREAMING_REQUEST_V1,
    method::STREAMING_ADMIT_V2,
    method::STREAMING_EVICT_V2,
    method::STREAMING_LIFECYCLE_V2,
    method::STREAMING_PIN_V1,
    method::STREAMING_UNPIN_V1,
    method::STREAMING_TOUCH_V1,
    method::STREAMING_CLEANUP_V1,
    method::STREAMING_COMPACT_V1,
    method::STREAMING_STATS_V1,
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

#[cfg(test)]
mod file_type_tests;
