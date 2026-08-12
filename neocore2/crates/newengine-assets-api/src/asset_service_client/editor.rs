use newengine_plugin_api::MethodName;

use super::AssetServiceClient;
use crate::{
    asset_edit_method, asset_inspect_method, AssetDocument, AssetDocumentRequest, AssetPatch,
    AssetPatchResult, ENGINE_ASSETS_EDIT_SERVICE_ID, ENGINE_ASSETS_INSPECT_SERVICE_ID,
};

impl AssetServiceClient {
    /// Inspect an asset document through the dedicated `engine.assets.inspect` gateway.
    ///
    /// This keeps editor UI callers away from format parsing and away from the
    /// root `engine.assets` byte/VFS surface: the selected inspect provider owns
    /// the normalized `AssetDocument` DTO.
    pub fn inspect_document_json_v1(
        &self,
        request: AssetDocumentRequest,
    ) -> Result<AssetDocument, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_INSPECT_SERVICE_ID,
            MethodName::from(asset_inspect_method::INSPECT_DOCUMENT_JSON_V1),
            &request,
            "inspect_document_json_v1",
        )
    }

    /// Validate an editor-produced asset patch through `engine.assets.edit`.
    pub fn validate_patch_json_v1(&self, patch: AssetPatch) -> Result<AssetPatchResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_EDIT_SERVICE_ID,
            MethodName::from(asset_edit_method::VALIDATE_PATCH_JSON_V1),
            &patch,
            "validate_patch_json_v1",
        )
    }

    /// Apply an editor-produced asset patch through `engine.assets.edit`.
    pub fn apply_patch_json_v1(&self, patch: AssetPatch) -> Result<AssetPatchResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_EDIT_SERVICE_ID,
            MethodName::from(asset_edit_method::APPLY_PATCH_JSON_V1),
            &patch,
            "apply_patch_json_v1",
        )
    }

    /// Stage an editor-produced patch without writing source bytes.
    pub fn stage_patch_json_v1(&self, patch: AssetPatch) -> Result<AssetPatchResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_EDIT_SERVICE_ID,
            MethodName::from(asset_edit_method::STAGE_PATCH_JSON_V1),
            &patch,
            "stage_patch_json_v1",
        )
    }

    /// Rebuild/commit all staged mutations for one logical asset container.
    pub fn rebuild_staged_json_v1(&self, asset_ref: &str) -> Result<AssetPatchResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_EDIT_SERVICE_ID,
            MethodName::from(asset_edit_method::REBUILD_JSON_V1),
            &serde_json::json!({ "asset_ref": asset_ref }),
            "rebuild_staged_json_v1",
        )
    }

    /// Discard all staged mutations for one logical asset container.
    pub fn discard_staged_json_v1(&self, asset_ref: &str) -> Result<AssetPatchResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_EDIT_SERVICE_ID,
            MethodName::from(asset_edit_method::DISCARD_STAGED_JSON_V1),
            &serde_json::json!({ "asset_ref": asset_ref }),
            "discard_staged_json_v1",
        )
    }

    /// Query staged/dirty state for one logical asset container.
    pub fn dirty_state_json_v1(&self, asset_ref: &str) -> Result<AssetPatchResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_EDIT_SERVICE_ID,
            MethodName::from(asset_edit_method::DIRTY_STATE_JSON_V1),
            &serde_json::json!({ "asset_ref": asset_ref }),
            "dirty_state_json_v1",
        )
    }
}
