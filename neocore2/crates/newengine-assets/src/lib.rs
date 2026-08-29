#![forbid(unsafe_op_in_unsafe_fn)]
pub mod asset_access;
pub mod asset_document_service;
pub mod asset_service_client;
pub mod asset_type_registry;
pub mod consts;
pub mod streaming;

pub use asset_document_service::{
    asset_document_edit_gateway_service, asset_document_inspect_gateway_service,
    register_asset_document_gateways_best_effort,
};
pub use asset_service_client::AssetServiceClient;
pub use asset_type_registry::{
    asset_types_gateway_service, asset_types_service_info,
    register_asset_type_descriptor_best_effort, register_asset_types_gateway_best_effort,
};
pub use newengine_assets_api::{
    asset_source_role, assets_ui_method, definitions_method, require_asset_reference_extension,
    wait_ready, AssetAccess, AssetDecodeRequest, AssetEntryDependency, AssetEntryManifest,
    AssetError, AssetErrorKind, AssetFileManifest, AssetFileTypeDescriptor, AssetFileTypeManifest,
    AssetFileTypeProbeRequest, AssetFileTypeProbeResult, AssetFileTypeRegisterRequest,
    AssetGatewayRoute, AssetReference, AssetResult, AssetService, AssetState,
    AssetStreamingCleanupRequestV1, AssetStreamingCleanupResponseV1, AssetStreamingPinClassV1,
    AssetStreamingPinRequestV1, AssetStreamingRequestV1, AssetStreamingStatsV1,
    AssetStreamingTouchRequestV1, Rgba8TextureAsset, RuntimeTextureAsset, RuntimeTextureFormat,
    RuntimeTextureMip, RuntimeTextureMipLayout, WaitReadyError, ASSETS_UI_BACKEND_CAPABILITY_ID,
    ASSETS_UI_RUNTIME_CONTRACT, ASSETS_UI_SERVICE_ID, ASSETS_UI_SERVICE_METHODS,
    ASSET_LIST_FILE_BODY_OUTPUT, ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1,
    ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, ENGINE_ASSETS_MATERIALS_SERVICE_ID,
    ENGINE_ASSETS_MODELS_SERVICE_ID, ENGINE_ASSETS_STREAMING_SERVICE_ID,
    ENGINE_ASSETS_TEXTURES_SERVICE_ID, ENGINE_ASSETS_UI_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
};
pub use streaming::AssetStreamingPinLease;

pub const ASSET_DOCUMENTS_RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.asset-documents",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[
            newengine_assets_api::ASSETS_INSPECT_BACKEND_CAPABILITY_ID,
            newengine_assets_api::ASSETS_EDIT_BACKEND_CAPABILITY_ID,
        ],
        &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn asset_documents_runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let _ = register_asset_document_gateways_best_effort(newengine_plugin_host::default_host_api());
    Ok(None)
}

pub const ASSET_DOCUMENTS_RUNTIME_UNIT_REGISTRATION:
    newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        ASSET_DOCUMENTS_RUNTIME_UNIT_SPEC,
        asset_documents_runtime_unit_factory,
    );
