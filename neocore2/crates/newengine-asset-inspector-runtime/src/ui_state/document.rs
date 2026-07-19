use super::*;

pub(super) fn publish_document_state(
    mut patch: UiStatePatch,
    document: Option<&AssetDocument>,
    container_available: bool,
    container_entry_count: usize,
) -> UiStatePatch {
    let Some(document) = document else {
        return patch
            .with_change("inspect", "visible", json!(false))
            .with_change("inspect", "empty_visible", json!(true))
            .with_change("inspect", "title", json!(""))
            .with_change("inspect", "asset_ref", json!(""))
            .with_change("inspect", "asset_kind", json!(""))
            .with_change("inspect", "document_kind", json!(""))
            .with_change("inspect", "provider", json!(""))
            .with_change("inspect", "edit_contract", json!(""))
            .with_change("inspect", "decoder", json!(""))
            .with_change("inspect", "summary", json!(""))
            .with_change("inspect", "can_apply_patch", json!(false))
            .with_change("inspect", "write_owner", json!(""))
            .with_change("preview", "container_available", json!(false))
            .with_change("preview", "container_entry_count", json!(0));
    };

    let provider = if document.provider_service.trim().is_empty() {
        document.semantic_gateway.as_str()
    } else {
        document.provider_service.as_str()
    };
    patch = patch
        .with_change("inspect", "visible", json!(true))
        .with_change("inspect", "empty_visible", json!(false))
        .with_change("inspect", "title", json!(document.title))
        .with_change("inspect", "asset_ref", json!(document.asset_ref))
        .with_change("inspect", "asset_kind", json!(document.asset_kind))
        .with_change("inspect", "document_kind", json!(document.document_kind))
        .with_change("inspect", "provider", json!(provider))
        .with_change("inspect", "edit_contract", json!(document.edit_contract))
        .with_change(
            "inspect",
            "decoder",
            json!(format!(
                "{} | inspect={} | schema={}",
                provider, document.inspect_contract, document.schema_contract
            )),
        )
        .with_change("inspect", "summary", json!(document.preview.summary))
        .with_change(
            "inspect",
            "can_apply_patch",
            json!(document.can_apply_patch),
        )
        .with_change("inspect", "write_owner", json!(document.write_owner))
        .with_change("preview", "container_available", json!(container_available))
        .with_change(
            "preview",
            "container_entry_count",
            json!(container_entry_count),
        )
        .with_change(
            "preview",
            "container_label",
            json!(if container_entry_count == 0 {
                "OPEN ENTRIES".to_owned()
            } else {
                format!("OPEN {} ENTRIES", container_entry_count)
            }),
        );
    patch
}

pub(super) fn publish_field_state(
    mut patch: UiStatePatch,
    document: Option<&AssetDocument>,
    modal_visible: bool,
) -> UiStatePatch {
    let fields = document
        .filter(|_| modal_visible)
        .into_iter()
        .flat_map(|document| {
            document.sections.iter().flat_map(|section| {
                section
                    .fields
                    .iter()
                    .map(move |field| (section.title.as_str(), field))
            })
        })
        .take(FIELD_ROWS)
        .collect::<Vec<_>>();
    let can_apply_patch = document.is_some_and(|document| document.can_apply_patch);

    for row in 0..FIELD_ROWS {
        let source = format!("field_{row:02}");
        if let Some((section, field)) = fields.get(row) {
            let editable = can_apply_patch && field.editable && field_has_pointer(field);
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "category", json!(section))
                .with_change(&source, "label", json!(field.label))
                .with_change(&source, "value", json!(editable_value(&field.value)))
                .with_change(&source, "editable", json!(editable))
                .with_change(&source, "readonly", json!(!editable))
                .with_change(&source, "value_kind", json!(field.value_kind))
                .with_change(&source, "help", json!(field.help));
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "category", json!(""))
                .with_change(&source, "label", json!(""))
                .with_change(&source, "value", json!(""))
                .with_change(&source, "editable", json!(false))
                .with_change(&source, "readonly", json!(true))
                .with_change(&source, "value_kind", json!(""))
                .with_change(&source, "help", json!(""));
        }
    }
    patch
}

pub(super) fn publish_action_state(
    mut patch: UiStatePatch,
    document: Option<&AssetDocument>,
) -> UiStatePatch {
    let available = document
        .into_iter()
        .flat_map(|document| document.actions.iter())
        .filter(|action| {
            action.enabled && !action.requires_input && action.patch_template.is_some()
        })
        .take(ACTION_ROWS)
        .collect::<Vec<_>>();
    let unavailable_count = document
        .map(|document| {
            document
                .actions
                .iter()
                .filter(|action| {
                    !(action.enabled && !action.requires_input && action.patch_template.is_some())
                })
                .count()
        })
        .unwrap_or(0);

    patch = patch
        .with_change(
            "actions",
            "summary",
            json!(if document.is_none() {
                "PROVIDER ACTIONS".to_owned()
            } else {
                format!("ACTIONS | {} READY", available.len())
            }),
        )
        .with_change(
            "actions",
            "empty_visible",
            json!(document.is_some() && available.is_empty()),
        )
        .with_change(
            "actions",
            "empty_text",
            json!(if unavailable_count > 0 {
                "No direct actions available | provider requires input or writer support"
            } else {
                "This provider exposes no document actions"
            }),
        );

    for row in 0..ACTION_ROWS {
        let source = format!("action_{row:02}");
        if let Some(action) = available.get(row) {
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "label", json!(action.label))
                .with_change(&source, "enabled", json!(true))
                .with_change(&source, "tooltip", json!(action.tooltip));
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "label", json!(""))
                .with_change(&source, "enabled", json!(false))
                .with_change(&source, "tooltip", json!(""));
        }
    }
    patch
}

pub(super) fn publish_diagnostics(
    mut patch: UiStatePatch,
    document: Option<&AssetDocument>,
    preview: Option<&AssetPreviewSnapshot>,
    patch_result: Option<&AssetPatchResult>,
    modal_visible: bool,
) -> UiStatePatch {
    let diagnostics = patch_result
        .filter(|_| modal_visible)
        .into_iter()
        .flat_map(|result| result.diagnostics.iter())
        .chain(
            document
                .filter(|_| modal_visible)
                .into_iter()
                .flat_map(|document| document.diagnostics.iter()),
        )
        .map(|diagnostic| {
            format!(
                "{:?} | {} | {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            )
        })
        .chain(
            preview
                .filter(|_| modal_visible)
                .and_then(|preview| preview.diagnostic.as_deref())
                .into_iter()
                .map(|message| format!("PREVIEW | {message}")),
        )
        .take(DIAGNOSTIC_ROWS)
        .collect::<Vec<_>>();

    patch = patch
        .with_change("diagnostics", "visible", json!(!diagnostics.is_empty()))
        .with_change(
            "diagnostics",
            "title",
            json!(format!("DIAGNOSTICS | {}", diagnostics.len())),
        );

    for row in 0..DIAGNOSTIC_ROWS {
        let source = format!("diagnostic_{row:02}");
        if let Some(message) = diagnostics.get(row) {
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "message", json!(message));
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "message", json!(""));
        }
    }
    patch
}

pub(super) fn editable_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
    }
}

pub(super) fn field_has_pointer(field: &AssetDocumentField) -> bool {
    field
        .schema_property
        .as_ref()
        .is_some_and(|property| !property.json_pointer.trim().is_empty())
        || !field.source_pointer.trim().is_empty()
}
