#![forbid(unsafe_op_in_unsafe_fn)]

pub mod asset_access;
pub mod asset_service_client;
pub mod consts;
pub mod file_type_registry;
pub mod semantic_gateways;

pub use asset_service_client::AssetServiceClient;
pub use file_type_registry::{asset_file_types_gateway_service, asset_file_types_service_info, register_asset_file_types_gateway_best_effort};
pub use semantic_gateways::register_definitions_gateway_best_effort;
pub use newengine_assets_api::{
    wait_ready, AssetAccess, AssetDecodeRequest, AssetEntryDependency, AssetEntryManifest,
    AssetError, AssetErrorKind, AssetFileManifest, AssetFileTypeDescriptor,
    AssetFileTypeManifest, AssetFileTypeProbeRequest, AssetFileTypeProbeResult,
    AssetFileTypeRegisterRequest, AssetGatewayRoute, AssetReference, AssetResult,
    AssetService, AssetState, Rgba8TextureAsset, RuntimeTextureAsset,
    RuntimeTextureFormat, RuntimeTextureMip, RuntimeTextureMipLayout, WaitReadyError,
    ENGINE_ASSET_SERVICE_ID, ENGINE_DEFINITIONS_SERVICE_ID, ENGINE_MATERIALS_SERVICE_ID,
    ENGINE_MODEL_SERVICE_ID, ENGINE_TEXTURES_SERVICE_ID,
};
