#![forbid(unsafe_op_in_unsafe_fn)]

pub mod asset_access;
pub mod asset_service_client;
pub mod consts;
pub mod file_type_registry;

pub use asset_service_client::AssetServiceClient;
pub use file_type_registry::{asset_file_types_gateway_service, asset_file_types_service_info, register_asset_file_types_gateway_best_effort};
pub use newengine_assets_api::{wait_ready, AssetAccess, AssetDecodeRequest, AssetError, AssetErrorKind, AssetFileTypeDescriptor, AssetFileTypeManifest, AssetFileTypeProbeRequest, AssetFileTypeProbeResult, AssetFileTypeRegisterRequest, AssetResult, AssetService, AssetState, Rgba8TextureAsset, RuntimeTextureAsset, RuntimeTextureFormat, RuntimeTextureMip, RuntimeTextureMipLayout, WaitReadyError};
