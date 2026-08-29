#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-owned asset file-type descriptor registry.
//!
//! Storage/probing, transport registration and path normalization are separate
//! modules. The public gateway API remains unchanged.

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    file_type_method, AssetFileTypeDescriptor, AssetFileTypeManifest, AssetFileTypeProbeRequest,
    AssetFileTypeProbeResult, AssetFileTypeRegisterRequest, ASSET_TYPES_BACKEND_CAPABILITY_ID,
    ASSET_TYPES_SERVICE_ID, ASSET_TYPES_SERVICE_METHODS, ENGINE_ASSET_TYPES_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod path;
mod service;
mod state;

pub use self::service::{
    asset_types_gateway_service, asset_types_service_info,
    register_asset_type_descriptor_best_effort, register_asset_types_gateway_best_effort,
};

use self::path::{normalize_logical_path, path_extension};

#[derive(Clone, Debug, Serialize)]
pub struct AssetTypesServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub registered_extensions: Vec<String>,
}

#[derive(Clone, Default)]
struct AssetTypeRegistryState {
    registry: BTreeMap<String, AssetFileTypeDescriptor>,
    /// Registered suffixes sorted longest-first for allocation-free probe lookup.
    extension_suffixes: Vec<String>,
}

#[cfg(test)]
mod tests;
