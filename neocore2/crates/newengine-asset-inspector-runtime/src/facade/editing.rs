use super::values::parse_field_value;
use super::*;

impl EngineAssetFacade {
    pub(crate) fn apply_field_edit(
        &self,
        document: &AssetDocument,
        field: &AssetDocumentField,
        payload: &Value,
    ) -> AssetPatchResult {
        if !field.editable {
            return rejected(
                &document.asset_ref,
                "asset.field.readonly",
                format!("field '{}' is read-only", field.label),
            );
        }
        if !document.can_apply_patch {
            return rejected(
                &document.asset_ref,
                "asset.writer.unavailable",
                if document.write_owner.trim().is_empty() {
                    "provider did not expose an asset writer".to_owned()
                } else {
                    document.write_owner.clone()
                },
            );
        }

        let Some(path) = field_pointer(field) else {
            return rejected(
                &document.asset_ref,
                "asset.field.pointer_missing",
                format!(
                    "field '{}' has no provider/schema source pointer; the facade will not invent one",
                    field.label
                ),
            );
        };
        let raw = payload.get("value").unwrap_or(payload);
        let value = match parse_field_value(field, raw) {
            Ok(value) => value,
            Err(error) => return rejected(&document.asset_ref, "asset.field.value_invalid", error),
        };

        let property_id = field
            .schema_property
            .as_ref()
            .map(|property| property.property_id.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| field.id.clone());
        let schema_operation = SchemaPatchOperationV1 {
            op: "replace".to_owned(),
            path: path.clone(),
            property_id,
            value: value.clone(),
            old_value: Some(field.value.clone()),
        };
        let undo_operation = SchemaPatchOperationV1 {
            op: "replace".to_owned(),
            path: path.clone(),
            property_id: schema_operation.property_id.clone(),
            value: field.value.clone(),
            old_value: Some(value.clone()),
        };
        let transaction_id = format!(
            "asset-inspector-{}",
            self.transaction_sequence.fetch_add(1, Ordering::Relaxed)
        );
        let target_type = document
            .schema_type
            .as_ref()
            .map(|schema| schema.type_id.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| document.asset_kind.clone());
        let schema_patch = SchemaPatchDtoV1 {
            target_type: target_type.clone(),
            target_ref: document.asset_ref.clone(),
            requester: ASSET_INSPECTOR_REQUESTER.to_owned(),
            transaction_id: transaction_id.clone(),
            operations: vec![schema_operation.clone()],
            ..SchemaPatchDtoV1::default()
        };
        let transaction = SchemaTransactionDtoV1 {
            transaction_id,
            target_type,
            target_ref: document.asset_ref.clone(),
            requester: ASSET_INSPECTOR_REQUESTER.to_owned(),
            reason: format!("Edit {}", field.label),
            operations: vec![schema_operation.clone()],
            undo_operations: vec![undo_operation],
            ..SchemaTransactionDtoV1::default()
        };
        let patch = AssetPatch {
            asset_ref: document.asset_ref.clone(),
            provider_service: document.provider_service.clone(),
            edit_contract: document.edit_contract.clone(),
            operations: vec![AssetPatchOperation {
                op: "replace".to_owned(),
                path,
                value,
                old_value: Some(field.value.clone()),
                schema_operation: Some(schema_operation),
            }],
            requester: ASSET_INSPECTOR_REQUESTER.to_owned(),
            schema_patch: Some(schema_patch),
            transaction: Some(transaction),
            ..AssetPatch::default()
        };

        if document
            .descriptor
            .as_ref()
            .is_some_and(|descriptor| descriptor.native_container)
            || document.asset_ref.contains('@')
        {
            self.validate_then_stage(patch)
        } else {
            self.validate_then_apply(patch)
        }
    }

    pub(crate) fn dispatch_document_action(
        &self,
        document: &AssetDocument,
        action: &AssetDocumentAction,
    ) -> AssetPatchResult {
        if !action.enabled {
            return rejected(
                &document.asset_ref,
                "asset.action.disabled",
                if action.disabled_reason.trim().is_empty() {
                    format!("action '{}' is disabled by provider policy", action.label)
                } else {
                    action.disabled_reason.clone()
                },
            );
        }
        if action.requires_input {
            return rejected(
                &document.asset_ref,
                "asset.action.requires_input",
                format!(
                    "action '{}' requires a provider-declared input dialog",
                    action.label
                ),
            );
        }
        let Some(mut patch) = action.patch_template.clone() else {
            return rejected(
                &document.asset_ref,
                "asset.action.patch_missing",
                format!("action '{}' has no provider patch template", action.label),
            );
        };
        if patch.asset_ref.trim().is_empty() {
            patch.asset_ref = document.asset_ref.clone();
        }
        if patch.provider_service.trim().is_empty() {
            patch.provider_service = document.provider_service.clone();
        }
        if patch.edit_contract.trim().is_empty() {
            patch.edit_contract = document.edit_contract.clone();
        }
        patch.requester = ASSET_INSPECTOR_REQUESTER.to_owned();
        match action.method.as_str() {
            asset_edit_method::STAGE_PATCH_JSON_V1 => self.validate_then_stage(patch),
            asset_edit_method::REBUILD_JSON_V1 => self
                .client
                .rebuild_staged_json_v1(&patch.asset_ref)
                .unwrap_or_else(|error| rejected(&patch.asset_ref, "asset.rebuild.failed", error)),
            asset_edit_method::DISCARD_STAGED_JSON_V1 => self
                .client
                .discard_staged_json_v1(&patch.asset_ref)
                .unwrap_or_else(|error| rejected(&patch.asset_ref, "asset.discard.failed", error)),
            _ => self.validate_then_apply(patch),
        }
    }

    fn validate_then_stage(&self, patch: AssetPatch) -> AssetPatchResult {
        self.validate_then_mutate(patch, "asset.patch.stage_failed", |client, patch| {
            client.stage_patch_json_v1(patch)
        })
    }

    fn validate_then_apply(&self, patch: AssetPatch) -> AssetPatchResult {
        self.validate_then_mutate(patch, "asset.patch.apply_failed", |client, patch| {
            client.apply_patch_json_v1(patch)
        })
    }

    fn validate_then_mutate(
        &self,
        patch: AssetPatch,
        failure_code: &str,
        mutate: impl FnOnce(&AssetServiceClient, AssetPatch) -> Result<AssetPatchResult, String>,
    ) -> AssetPatchResult {
        let validation = match self.client.validate_patch_json_v1(patch.clone()) {
            Ok(result) => result,
            Err(error) => {
                return rejected(&patch.asset_ref, "asset.patch.validation_failed", error)
            }
        };
        if !validation.accepted {
            return validation;
        }
        match mutate(&self.client, patch) {
            Ok(mut result) => {
                let mut diagnostics = validation.diagnostics;
                diagnostics.append(&mut result.diagnostics);
                result.diagnostics = diagnostics;
                result
            }
            Err(error) => rejected(&validation.asset_ref, failure_code, error),
        }
    }
}

pub(super) fn field_pointer(field: &AssetDocumentField) -> Option<String> {
    field
        .schema_property
        .as_ref()
        .map(|property| property.json_pointer.trim())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let pointer = field.source_pointer.trim();
            (!pointer.is_empty()).then_some(pointer)
        })
        .map(ToOwned::to_owned)
}

fn rejected(asset_ref: &str, code: &str, message: impl Into<String>) -> AssetPatchResult {
    AssetPatchResult {
        asset_ref: asset_ref.to_owned(),
        accepted: false,
        written: false,
        dirty: false,
        diagnostics: vec![AssetDocumentDiagnostic::error(code, message)],
        ..AssetPatchResult::default()
    }
}
