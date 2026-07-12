use super::*;

pub(super) fn asset_document_actions(
    document: &AssetDocument,
    selected_entry: Option<&str>,
) -> Vec<AssetDocumentAction> {
    let mut actions = Vec::new();
    let target_ref = normalize_asset_ref(&document.asset_ref);
    let can_write = document.can_apply_patch;
    let has_entry_selection = selected_entry.is_some() || target_ref.contains('@');

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::ADD_ENTRY.to_owned(),
        label: "Add".to_owned(),
        tooltip: "Add a new entry through the selected file type provider. Requires a source/template payload before AssetPatch can be emitted.".to_owned(),
        enabled: false,
        disabled_reason: if can_write { "requires add-entry dialog/schema payload" } else { "writer capability unavailable" }.to_owned(),
        requires_input: true,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: Some(asset_action_input_schema(
            "newengine.assets.action.add_entry.input.v1",
            "Add Entry Input",
            vec![
                schema_property("entry_name", "Entry Name", SchemaValueKindV1::String, json!(""), true),
                schema_property("template_kind", "Template Kind", SchemaValueKindV1::Enum, json!(document.asset_kind.clone()), true),
            ],
        )),
    });

    let delete_patch = if can_write && has_entry_selection {
        let schema_op = SchemaPatchOperationV1 {
            op: "delete".to_owned(),
            path: "/entries/current".to_owned(),
            property_id: "entries.current".to_owned(),
            value: Value::Null,
            old_value: None,
        };
        Some(AssetPatch {
            asset_ref: target_ref.clone(),
            provider_service: document.provider_service.clone(),
            edit_contract: document.edit_contract.clone(),
            requester: "ui.assets.catalog".to_owned(),
            operations: vec![AssetPatchOperation {
                op: "delete".to_owned(),
                path: "/entries/current".to_owned(),
                value: Value::Null,
                old_value: None,
                schema_operation: Some(schema_op.clone()),
            }],
            schema_patch: Some(schema_patch_for_document(
                document,
                &target_ref,
                "asset-delete",
                vec![schema_op.clone()],
            )),
            transaction: Some(schema_transaction_for_document(
                document,
                &target_ref,
                "asset-delete",
                vec![schema_op],
                Vec::new(),
            )),
            ..AssetPatch::default()
        })
    } else {
        None
    };
    actions.push(AssetDocumentAction {
        id: asset_document_action_id::DELETE.to_owned(),
        label: "Delete".to_owned(),
        tooltip: "Delete the selected ListFile entry through engine.assets.edit.".to_owned(),
        enabled: delete_patch.is_some(),
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !has_entry_selection {
            "select a file@entry item first".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: delete_patch,
        input_schema: None,
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::RENAME.to_owned(),
        label: "Rename".to_owned(),
        tooltip: "Rename the selected entry. This action requires a rename dialog before a provider patch can be emitted.".to_owned(),
        enabled: false,
        disabled_reason: if can_write && has_entry_selection { "requires rename dialog value" } else { "select writable file@entry item first" }.to_owned(),
        requires_input: true,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: Some(asset_action_input_schema(
            "newengine.assets.action.rename.input.v1",
            "Rename Entry Input",
            vec![schema_property("new_name", "New Name", SchemaValueKindV1::String, json!(selected_entry.unwrap_or("")), true)],
        )),
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::SAVE.to_owned(),
        label: "Save".to_owned(),
        tooltip: "Apply the current dirty AssetPatch through engine.assets.edit.".to_owned(),
        enabled: can_write && document.dirty,
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !document.dirty {
            "no dirty AssetPatch to save".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: if can_write && document.dirty {
            Some(AssetPatch {
                asset_ref: target_ref.clone(),
                provider_service: document.provider_service.clone(),
                edit_contract: document.edit_contract.clone(),
                requester: "ui.assets.catalog".to_owned(),
                operations: Vec::new(),
                schema_patch: Some(schema_patch_for_document(
                    document,
                    &target_ref,
                    "asset-save",
                    Vec::new(),
                )),
                transaction: Some(schema_transaction_for_document(
                    document,
                    &target_ref,
                    "asset-save",
                    Vec::new(),
                    Vec::new(),
                )),
                ..AssetPatch::default()
            })
        } else {
            None
        },
        input_schema: None,
    });

    actions
}
