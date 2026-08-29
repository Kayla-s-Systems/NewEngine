#![forbid(unsafe_op_in_unsafe_fn)]

//! Schema-driven asset document inspection and editing gateways.
//!
//! The facade keeps the historical public API stable while implementation is
//! split by responsibility: inspection, edit validation/writeback, document
//! sections/actions/schema, path normalization and service transport.

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    asset_document_action_id, asset_edit_method, asset_inspect_method, file_type_method,
    AssetAccess, AssetDecodeRequest, AssetDocument, AssetDocumentAction, AssetDocumentDiagnostic,
    AssetDocumentField, AssetDocumentPreview, AssetDocumentRequest, AssetDocumentSection,
    AssetDocumentText, AssetFileManifest, AssetFileTypeDescriptor, AssetFileTypeProbeRequest,
    AssetFileTypeProbeResult, AssetPatch, AssetPatchOperation, AssetPatchResult, AssetService,
    AssetServiceClient, ASSETS_EDIT_BACKEND_CAPABILITY_ID, ASSETS_EDIT_SERVICE_ID,
    ASSETS_EDIT_SERVICE_METHODS, ASSETS_INSPECT_BACKEND_CAPABILITY_ID, ASSETS_INSPECT_SERVICE_ID,
    ASSETS_INSPECT_SERVICE_METHODS, ASSETS_PACKAGE_WRITER_CAPABILITY_ID,
    ASSET_LIST_FILE_MANIFEST_OUTPUT, ENGINE_ASSETS_EDIT_SERVICE_ID,
    ENGINE_ASSETS_INSPECT_SERVICE_ID, ENGINE_ASSET_TYPES_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_schema_api::{
    SchemaPatchDtoV1, SchemaPatchOperationV1, SchemaPropertyDescriptorV1, SchemaTransactionDtoV1,
    SchemaTypeDescriptorV1, SchemaValueKindV1, SCHEMA_RUNTIME_CONTRACT,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_dynamic_best_effort, EngineGatewayProviderDeclDynamic,
    JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

mod actions;
mod edit;
mod inspect;
mod path;
mod schema;
mod sections;
mod transport;

pub use self::transport::{
    asset_document_edit_gateway_service, asset_document_inspect_gateway_service,
    register_asset_document_gateways_best_effort,
};

use self::actions::*;
use self::path::*;
use self::schema::*;
use self::sections::*;

#[derive(Clone, Debug, Serialize)]
pub struct AssetInspectServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub policy: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssetEditServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub policy: &'static str,
}

#[derive(Clone)]
struct AssetInspectState {
    host: HostApiV1,
    assets: AssetServiceClient,
    asset_types_service_id: RString,
    resolve_method: MethodName,
}

#[derive(Clone)]
struct AssetEditState {
    assets: AssetServiceClient,
    staged: Arc<Mutex<BTreeMap<String, Vec<AssetPatch>>>>,
}

#[cfg(test)]
mod tests;
