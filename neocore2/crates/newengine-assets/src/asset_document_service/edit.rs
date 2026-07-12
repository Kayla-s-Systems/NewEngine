use super::*;

impl AssetEditState {
    pub(super) fn new(host: HostApiV1) -> Self {
        Self {
            assets: AssetServiceClient::new(host),
        }
    }

    pub(super) fn validate_patch(&self, patch: AssetPatch) -> AssetPatchResult {
        let mut result = AssetPatchResult {
            asset_ref: normalize_asset_ref(&patch.asset_ref),
            ..AssetPatchResult::default()
        };
        if result.asset_ref.is_empty() {
            result.diagnostics.push(AssetDocumentDiagnostic::error(
                "asset_ref.empty",
                "patch requires asset_ref",
            ));
            return result;
        }
        if let Some(schema_patch) = patch.schema_patch.as_ref() {
            if schema_patch.target_ref.trim().is_empty()
                || schema_patch.target_ref != result.asset_ref
            {
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "schema.patch.target_mismatch",
                    "schema_patch.target_ref must match AssetPatch.asset_ref before provider validation",
                ));
                return result;
            }
            if schema_patch.operations.len() != patch.operations.len() {
                result.diagnostics.push(AssetDocumentDiagnostic::warn(
                    "schema.patch.operation_projection",
                    "schema_patch operation count differs from transport operations; provider will validate canonical schema operations first",
                ));
            }
        } else {
            result.diagnostics.push(AssetDocumentDiagnostic::warn(
                "schema.patch.missing",
                "AssetPatch has no SchemaPatchDtoV1 projection; accepting legacy transport only for compatibility during P2 migration",
            ));
        }
        if patch.transaction.is_none() {
            result.diagnostics.push(AssetDocumentDiagnostic::warn(
                "schema.transaction.missing",
                "AssetPatch has no SchemaTransactionDtoV1; undo/redo history will not be able to replay this change through engine.schema",
            ));
        }
        if patch.operations.is_empty() {
            result.accepted = true;
            result.diagnostics.push(AssetDocumentDiagnostic::info(
                "patch.empty",
                "empty patch is valid and has no write effect",
            ));
            return result;
        }
        if patch.edit_contract.trim().is_empty()
            || patch.edit_contract == newengine_assets_api::ASSETS_EDIT_RUNTIME_CONTRACT
        {
            result.diagnostics.push(AssetDocumentDiagnostic::warn(
                "edit.contract.generic",
                "generic asset edit provider can validate transport only; format provider must supply an explicit edit_contract before write-back",
            ));
            result.accepted = false;
            return result;
        }
        result.accepted = true;
        result.dirty = true;
        result.diagnostics.push(AssetDocumentDiagnostic::info(
            "patch.accepted",
            "patch transport is valid; provider-specific writer owns final validation",
        ));
        result
    }

    pub(super) fn apply_patch(&self, patch: AssetPatch) -> AssetPatchResult {
        let mut result = self.validate_patch(patch.clone());
        if !result.accepted {
            return result;
        }

        let Some(first_op) = patch.operations.first() else {
            result.written = false;
            result.dirty = false;
            return result;
        };

        let operation = match first_op.op.trim().to_ascii_lowercase().as_str() {
            "remove" | "delete" => "delete",
            "rename" => "rename",
            "add" | "create" | "replace" | "update" => "update",
            _ => {
                result.accepted = false;
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "patch.operation.unsupported",
                    format!("unsupported asset patch operation '{}'", first_op.op),
                ));
                return result;
            }
        };

        let client = &self.assets;
        let mut payload = json!({
            "target_ref": result.asset_ref,
            "operation": operation,
            "verify_after_build": true,
            "dry_run": false,
        });
        if operation == "update" {
            payload["payload_json"] = first_op.value.clone();
        }
        if operation == "rename" {
            if let Some(new_name) = first_op.value.as_str() {
                payload["new_name"] = json!(new_name);
            } else if let Some(new_name) = first_op.value.get("name").and_then(Value::as_str) {
                payload["new_name"] = json!(new_name);
            }
        }

        match client.list_file_repack_json_v1(payload) {
            Ok(value) => {
                let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let applied = value
                    .get("applied")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                result.accepted = ok;
                result.written = applied;
                result.dirty = !applied;
                result.diagnostics.push(AssetDocumentDiagnostic::info(
                    "listfile.repack",
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("NEF8 ListFile writer completed"),
                ));
                if !ok {
                    result.diagnostics.push(AssetDocumentDiagnostic::warn(
                        "listfile.repack.not_applied",
                        "writer rejected or dry-ran the patch",
                    ));
                }
                result
            }
            Err(error) => {
                result.accepted = false;
                result.written = false;
                result.dirty = true;
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "listfile.repack.failed",
                    error,
                ));
                result
            }
        }
    }
}
