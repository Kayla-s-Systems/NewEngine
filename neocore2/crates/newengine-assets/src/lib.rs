#![forbid(unsafe_op_in_unsafe_fn)]

pub mod asset_access;
pub mod asset_service_client;
pub mod consts;

pub use asset_service_client::AssetServiceClient;
pub use newengine_assets_api::{wait_ready, AssetAccess, AssetError, AssetErrorKind, AssetResult, AssetService, AssetState, Rgba8TextureAsset, RuntimeTextureAsset, RuntimeTextureFormat, RuntimeTextureMip, RuntimeTextureMipLayout, WaitReadyError};
