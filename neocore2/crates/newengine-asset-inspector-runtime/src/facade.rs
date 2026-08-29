use std::sync::atomic::{AtomicU64, Ordering};

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_assets_api::{
    asset_edit_method, AssetDecodeRequest, AssetDocument, AssetDocumentAction,
    AssetDocumentDiagnostic, AssetDocumentField, AssetDocumentRequest, AssetFileManifest,
    AssetPatch, AssetPatchOperation, AssetPatchResult, TextAssetWriteRequestV1,
    TextAssetWriteResponseV1, ASSETS_PACKAGE_WRITER_CAPABILITY_ID, ASSET_LIST_FILE_MANIFEST_OUTPUT,
};
use newengine_schema_api::{
    SchemaPatchDtoV1, SchemaPatchOperationV1, SchemaTransactionDtoV1, SchemaValueKindV1,
};
use serde_json::{json, Value};

use crate::model::InspectorEntry;

pub(crate) const ASSET_INSPECTOR_REQUESTER: &str = "app.asset_inspector";

pub(crate) struct EngineAssetFacade {
    client: AssetServiceClient,
    transaction_sequence: AtomicU64,
}

impl Default for EngineAssetFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineAssetFacade {
    pub(crate) fn new() -> Self {
        Self {
            client: AssetServiceClient::new(newengine_plugin_host::default_host_api()),
            transaction_sequence: AtomicU64::new(1),
        }
    }
}

mod editing;
mod listing;
#[cfg(test)]
mod tests;
mod values;
