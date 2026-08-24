#![forbid(unsafe_op_in_unsafe_fn)]
pub mod asset_access;
pub mod asset_document_service;
pub mod asset_service_client;
pub mod asset_type_registry;
pub mod consts;

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
    AssetGatewayRoute, AssetReference, AssetResult, AssetService, AssetState, Rgba8TextureAsset,
    RuntimeTextureAsset, RuntimeTextureFormat, RuntimeTextureMip, RuntimeTextureMipLayout,
    WaitReadyError, ASSETS_UI_BACKEND_CAPABILITY_ID, ASSETS_UI_RUNTIME_CONTRACT,
    ASSETS_UI_SERVICE_ID, ASSETS_UI_SERVICE_METHODS, ASSET_LIST_FILE_BODY_OUTPUT,
    ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1,
    ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, ENGINE_ASSETS_MATERIALS_SERVICE_ID,
    ENGINE_ASSETS_MODELS_SERVICE_ID, ENGINE_ASSETS_TEXTURES_SERVICE_ID,
    ENGINE_ASSETS_UI_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
};
