use super::*;

impl AssetEditState {
    pub(super) fn new(host: HostApiV1) -> Self {
        Self {
            assets: AssetServiceClient::new(host),
            staged: Arc::new(Mutex::new(BTreeMap::new())),
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
        for operation in &patch.operations {
            if normalized_operation(&operation.op).is_none() {
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "patch.operation.unsupported",
                    format!("unsupported asset patch operation '{}'", operation.op),
                ));
                return result;
            }
        }
        result.accepted = true;
        result.dirty = true;
        result.diagnostics.push(AssetDocumentDiagnostic::info(
            "patch.accepted",
            "patch transport is valid; provider-specific writer owns final validation",
        ));
        result
    }

    /// Stage a mutation in the engine-owned edit session without touching VFS bytes.
    pub(super) fn stage_patch(&self, patch: AssetPatch) -> AssetPatchResult {
        let mut result = self.validate_patch(patch.clone());
        if !result.accepted || patch.operations.is_empty() {
            return result;
        }
        let (logical_path, _) = split_entry_ref(&result.asset_ref);
        let mut staged = match self.staged.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        staged.entry(logical_path.clone()).or_default().push(patch);
        result.asset_ref = logical_path;
        result.written = false;
        result.dirty = true;
        result.staged_operations = staged_operation_count(&staged, &result.asset_ref);
        result.staged_patches = staged.get(&result.asset_ref).cloned().unwrap_or_default();
        result.diagnostics.push(AssetDocumentDiagnostic::info(
            "patch.staged",
            format!(
                "mutation staged in engine.assets.edit; {} operation(s) await Rebuild",
                result.staged_operations
            ),
        ));
        result
    }

    pub(super) fn dirty_state(&self, payload: Value) -> AssetPatchResult {
        let asset_ref = request_asset_ref(&payload);
        let (logical_path, _) = split_entry_ref(&asset_ref);
        let staged = match self.staged.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let count = staged_operation_count(&staged, &logical_path);
        let staged_patches = staged.get(&logical_path).cloned().unwrap_or_default();
        AssetPatchResult {
            asset_ref: logical_path,
            accepted: true,
            written: false,
            dirty: count > 0,
            staged_operations: count,
            staged_patches,
            diagnostics: vec![AssetDocumentDiagnostic::info(
                "edit.session.state",
                format!("{count} staged operation(s)"),
            )],
            ..AssetPatchResult::default()
        }
    }

    pub(super) fn discard_staged(&self, payload: Value) -> AssetPatchResult {
        let asset_ref = request_asset_ref(&payload);
        let (logical_path, _) = split_entry_ref(&asset_ref);
        let mut staged = match self.staged.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let discarded = staged
            .remove(&logical_path)
            .map(|patches| patches.iter().map(|patch| patch.operations.len()).sum())
            .unwrap_or(0usize);
        AssetPatchResult {
            asset_ref: logical_path,
            accepted: true,
            written: false,
            dirty: false,
            staged_operations: 0,
            diagnostics: vec![AssetDocumentDiagnostic::info(
                "edit.session.discarded",
                format!("discarded {discarded} staged operation(s)"),
            )],
            ..AssetPatchResult::default()
        }
    }

    /// Commit all staged operations with one provider-owned rebuild/repack call.
    pub(super) fn rebuild_staged(&self, payload: Value) -> AssetPatchResult {
        let asset_ref = request_asset_ref(&payload);
        let (logical_path, _) = split_entry_ref(&asset_ref);
        let patches = {
            let staged = match self.staged.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            staged.get(&logical_path).cloned().unwrap_or_default()
        };
        let staged_operations = patches.iter().map(|patch| patch.operations.len()).sum();
        if staged_operations == 0 {
            return AssetPatchResult {
                asset_ref: logical_path,
                accepted: true,
                written: false,
                dirty: false,
                staged_operations: 0,
                diagnostics: vec![AssetDocumentDiagnostic::info(
                    "edit.session.clean",
                    "no staged mutations to rebuild",
                )],
                ..AssetPatchResult::default()
            };
        }

        let mutations = patches
            .iter()
            .flat_map(|patch| {
                patch
                    .operations
                    .iter()
                    .filter_map(|operation| mutation_payload(&patch.asset_ref, operation))
            })
            .collect::<Vec<_>>();
        if mutations.len() != staged_operations {
            return AssetPatchResult {
                asset_ref: logical_path,
                accepted: false,
                written: false,
                dirty: true,
                staged_operations,
                diagnostics: vec![AssetDocumentDiagnostic::error(
                    "edit.session.invalid_mutation",
                    "one or more staged operations could not be projected into provider mutation DTOs",
                )],
                ..AssetPatchResult::default()
            };
        }

        let writer_payload = json!({
            "logical_path": logical_path,
            "operation": "rebuild",
            "mutations": mutations,
            "verify_after_build": true,
            "dry_run": false,
        });
        match self.assets.list_file_repack_json_v1(writer_payload) {
            Ok(value) => {
                let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let applied = value
                    .get("applied")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if ok && applied {
                    let mut staged = match self.staged.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    staged.remove(&logical_path);
                }
                AssetPatchResult {
                    asset_ref: logical_path,
                    accepted: ok,
                    written: applied,
                    dirty: !(ok && applied),
                    staged_operations: if ok && applied { 0 } else { staged_operations },
                    diagnostics: vec![if ok && applied {
                        AssetDocumentDiagnostic::info(
                            "asset.rebuild.completed",
                            value
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("asset container rebuilt and written"),
                        )
                    } else {
                        AssetDocumentDiagnostic::error(
                            "asset.rebuild.failed",
                            value
                                .get("message")
                                .or_else(|| value.get("error"))
                                .and_then(Value::as_str)
                                .unwrap_or("asset writer rejected rebuild"),
                        )
                    }],
                    ..AssetPatchResult::default()
                }
            }
            Err(error) => AssetPatchResult {
                asset_ref: logical_path,
                accepted: false,
                written: false,
                dirty: true,
                staged_operations,
                diagnostics: vec![AssetDocumentDiagnostic::error(
                    "asset.rebuild.transport_failed",
                    error,
                )],
                ..AssetPatchResult::default()
            },
        }
    }

    /// Immediate write path retained for callers that explicitly request Apply.
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
        let Some(operation) = normalized_operation(&first_op.op) else {
            result.accepted = false;
            result.diagnostics.push(AssetDocumentDiagnostic::error(
                "patch.operation.unsupported",
                format!("unsupported asset patch operation '{}'", first_op.op),
            ));
            return result;
        };

        let mut payload = mutation_payload(&patch.asset_ref, first_op).unwrap_or_else(|| {
            json!({
                "target_ref": result.asset_ref,
                "operation": operation,
            })
        });
        payload["verify_after_build"] = json!(true);
        payload["dry_run"] = json!(false);

        match self.assets.list_file_repack_json_v1(payload) {
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
                        .unwrap_or("asset writer completed"),
                ));
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

fn normalized_operation(operation: &str) -> Option<&'static str> {
    match operation.trim().to_ascii_lowercase().as_str() {
        "remove" | "delete" => Some("delete"),
        "rename" => Some("rename"),
        "add" | "create" | "replace" | "update" => Some("update"),
        "rebuild" | "repack" => Some("rebuild"),
        _ => None,
    }
}

fn request_asset_ref(payload: &Value) -> String {
    normalize_asset_ref(
        payload
            .get("asset_ref")
            .or_else(|| payload.get("target_ref"))
            .or_else(|| payload.get("logical_path"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}

fn staged_operation_count(staged: &BTreeMap<String, Vec<AssetPatch>>, logical_path: &str) -> usize {
    staged
        .get(logical_path)
        .map(|patches| patches.iter().map(|patch| patch.operations.len()).sum())
        .unwrap_or(0)
}

fn mutation_payload(asset_ref: &str, operation: &AssetPatchOperation) -> Option<Value> {
    let kind = normalized_operation(&operation.op)?;
    let mut payload = json!({
        "target_ref": normalize_asset_ref(asset_ref),
        "operation": kind,
    });
    if kind == "update" {
        payload["payload_json"] = operation.value.clone();
    } else if kind == "rename" {
        if let Some(name) = operation.value.as_str() {
            payload["new_name"] = json!(name);
        } else if let Some(name) = operation.value.get("name").and_then(Value::as_str) {
            payload["new_name"] = json!(name);
        }
    }
    Some(payload)
}
