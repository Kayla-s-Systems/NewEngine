use super::super::document::available_document_action;
use super::super::interaction::parse_index;
use super::super::*;

#[test]
fn parses_authored_entry_and_field_row_ids() {
    assert_eq!(
        parse_index(
            "asset.inspector.entry.07",
            "asset.inspector.entry.",
            ENTRY_ROWS
        ),
        Some(7)
    );
    assert_eq!(
        parse_index(
            "asset.inspector.field.03.input",
            "asset.inspector.field.",
            FIELD_ROWS
        ),
        Some(3)
    );
}

#[test]
fn hover_exit_only_clears_the_matching_hover_owner() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_engine_runtime::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);

    runtime.handle_hover("asset.inspector.refresh", UiNodeEventTrigger::HoverEnter);
    assert_eq!(
        runtime.hovered_node.as_deref(),
        Some("asset.inspector.refresh")
    );
    assert!(runtime.hover_hint.contains("Refresh"));

    runtime.handle_hover("asset.inspector.up", UiNodeEventTrigger::HoverExit);
    assert_eq!(
        runtime.hovered_node.as_deref(),
        Some("asset.inspector.refresh")
    );
    assert!(!runtime.hover_hint.is_empty());

    runtime.handle_hover("asset.inspector.refresh", UiNodeEventTrigger::HoverExit);
    assert!(runtime.hovered_node.is_none());
    assert!(runtime.hover_hint.is_empty());
}

#[test]
fn field_hover_uses_provider_help_text() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_engine_runtime::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.document = Some(AssetDocument {
        sections: vec![newengine_assets_api::AssetDocumentSection {
            title: "Identity".to_owned(),
            fields: vec![AssetDocumentField {
                label: "Asset Ref".to_owned(),
                help: "Canonical provider-owned reference".to_owned(),
                ..AssetDocumentField::default()
            }],
            ..Default::default()
        }],
        ..AssetDocument::default()
    });

    runtime.handle_hover("asset.inspector.field.00", UiNodeEventTrigger::HoverEnter);
    assert_eq!(
        runtime.hover_hint,
        "Asset Ref | Canonical provider-owned reference"
    );
}

#[test]
fn available_document_actions_are_compacted_for_ui_rows() {
    let disabled = newengine_assets_api::AssetDocumentAction {
        label: "Unavailable".to_owned(),
        enabled: false,
        ..Default::default()
    };
    let available = newengine_assets_api::AssetDocumentAction {
        label: "Rebuild".to_owned(),
        tooltip: "Rebuild staged provider mutations".to_owned(),
        enabled: true,
        patch_template: Some(newengine_assets_api::AssetPatch::default()),
        ..Default::default()
    };
    let document = AssetDocument {
        actions: vec![disabled, available],
        ..AssetDocument::default()
    };

    assert_eq!(
        available_document_action(&document, 0).map(|action| action.label.as_str()),
        Some("Rebuild")
    );
    assert!(available_document_action(&document, 1).is_none());
}
