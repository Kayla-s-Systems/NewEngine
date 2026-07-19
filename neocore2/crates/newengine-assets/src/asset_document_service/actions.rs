use super::*;

pub(super) fn asset_document_actions(
    document: &AssetDocument,
    selected_entry: Option<&str>,
) -> Vec<AssetDocumentAction> {
    let mut actions = Vec::new();
    let target_ref = normalize_asset_ref(&document.asset_ref);
    let (container_ref, _) = split_entry_ref(&target_ref);
    let can_write = document.can_apply_patch;
    let has_entry_selection = selected_entry.is_some() || target_ref.contains('@');

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::ADD_ENTRY.to_owned(),
        label: "Add".to_owned(),
        tooltip: "Stage a new entry through the selected file type provider. Rebuild commits all staged changes.".to_owned(),
        enabled: false,
        disabled_reason: if can_write {
            "requires provider-declared add-entry input"
        } else {
            "writer capability unavailable"
        }
        .to_owned(),
        requires_input: true,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::STAGE_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: Some(asset_action_input_schema(
            "newengine.assets.action.add_entry.input.v1",
            "Add Entry Input",
            vec![
                schema_property(
                    "entry_name",
                    "Entry Name",
                    SchemaValueKindV1::String,
                    json!(""),
                    true,
                ),
                schema_property(
                    "template_kind",
                    "Template Kind",
                    SchemaValueKindV1::Enum,
                    json!(document.asset_kind.clone()),
                    true,
                ),
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
            requester: "ui.assets.inspector".to_owned(),
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
                "asset-delete-stage",
                vec![schema_op.clone()],
            )),
            transaction: Some(schema_transaction_for_document(
                document,
                &target_ref,
                "asset-delete-stage",
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
        label: "Delete Entry".to_owned(),
        tooltip: "Stage deletion of the selected container entry. Source bytes remain unchanged until Rebuild.".to_owned(),
        enabled: delete_patch.is_some(),
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !has_entry_selection {
            "select a container entry first".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::STAGE_PATCH_JSON_V1.to_owned(),
        patch_template: delete_patch,
        input_schema: None,
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::RENAME.to_owned(),
        label: "Rename".to_owned(),
        tooltip: "Stage rename of the selected entry. Rebuild commits the transaction.".to_owned(),
        enabled: false,
        disabled_reason: if can_write && has_entry_selection {
            "requires provider-declared rename input"
        } else {
            "select a writable container entry first"
        }
        .to_owned(),
        requires_input: true,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::STAGE_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: Some(asset_action_input_schema(
            "newengine.assets.action.rename.input.v1",
            "Rename Entry Input",
            vec![schema_property(
                "new_name",
                "New Name",
                SchemaValueKindV1::String,
                json!(selected_entry.unwrap_or("")),
                true,
            )],
        )),
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::REBUILD.to_owned(),
        label: "Rebuild".to_owned(),
        tooltip: "Validate and atomically rebuild the container from all staged mutations."
            .to_owned(),
        enabled: can_write && document.dirty,
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !document.dirty {
            "container has no staged mutations".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::REBUILD_JSON_V1.to_owned(),
        patch_template: Some(AssetPatch {
            asset_ref: container_ref.clone(),
            provider_service: document.provider_service.clone(),
            edit_contract: document.edit_contract.clone(),
            requester: "ui.assets.inspector".to_owned(),
            ..AssetPatch::default()
        }),
        input_schema: None,
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::DISCARD.to_owned(),
        label: "Discard".to_owned(),
        tooltip: "Discard every staged mutation for this container without changing source bytes."
            .to_owned(),
        enabled: document.dirty,
        disabled_reason: if document.dirty {
            String::new()
        } else {
            "container has no staged mutations".to_owned()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::DISCARD_STAGED_JSON_V1.to_owned(),
        patch_template: Some(AssetPatch {
            asset_ref: container_ref.clone(),
            requester: "ui.assets.inspector".to_owned(),
            ..AssetPatch::default()
        }),
        input_schema: None,
    });

    // Immediate save remains available for non-container providers that expose a
    // dirty document model instead of a staged rebuild workflow.
    actions.push(AssetDocumentAction {
        id: asset_document_action_id::SAVE.to_owned(),
        label: "Save".to_owned(),
        tooltip: "Apply the current provider document patch immediately.".to_owned(),
        enabled: can_write && document.dirty && !has_entry_selection,
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !document.dirty {
            "document is clean".to_owned()
        } else if has_entry_selection {
            "container entries are committed with Rebuild".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: None,
    });

    actions
}
