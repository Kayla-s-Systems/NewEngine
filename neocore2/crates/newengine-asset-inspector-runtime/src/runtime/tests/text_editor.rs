use super::super::*;

#[test]
fn text_editor_preserves_crlf_and_trailing_newline() {
    let document = AssetDocument {
        asset_ref: "config/test.json".to_owned(),
        text: Some(newengine_assets_api::AssetDocumentText {
            content: "a\r\nb\r\n".to_owned(),
            language: "json".to_owned(),
            editable: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let editor = TextEditorState::from_document(&document).unwrap();
    assert_eq!(editor.lines, vec!["a", "b", ""]);
    assert_eq!(editor.compose(), "a\r\nb\r\n");
}

#[test]
fn text_document_builds_preview_and_editor_syntax_pages() {
    let document = AssetDocument {
        asset_ref: "config/runtime.json".to_owned(),
        text: Some(newengine_assets_api::AssetDocumentText {
            content: "{\n  \"enabled\": true,\n  \"count\": 42\n}\n".to_owned(),
            language: "json".to_owned(),
            editable: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.sync_text_editor_from_document(&document);

    assert!(runtime.text_editor.is_some());
    assert!(runtime.syntax_preview.is_some());
    assert!(runtime.syntax_editor.is_some());
    assert_eq!(runtime.syntax_editor.as_ref().unwrap().language, "json");
    assert!(runtime.syntax_editor.as_ref().unwrap().rows[1].layers
        [crate::syntax_preview::SyntaxClass::Attribute as usize]
        .contains("enabled"));
}

#[test]
fn text_line_edit_marks_document_dirty() {
    let document = AssetDocument {
        asset_ref: "config/test.json".to_owned(),
        text: Some(newengine_assets_api::AssetDocumentText {
            content: "old".to_owned(),
            editable: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.text_editor = TextEditorState::from_document(&document);
    runtime.edit_text_line(0, &serde_json::json!({"value": "new"}));
    assert_eq!(runtime.text_editor.as_ref().unwrap().compose(), "new");
    assert!(runtime.text_editor.as_ref().unwrap().dirty);
}
