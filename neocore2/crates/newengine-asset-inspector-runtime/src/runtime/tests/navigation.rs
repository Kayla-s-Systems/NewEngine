use super::super::navigation::prepend_parent_navigation;
use super::super::*;

#[test]
fn parent_navigation_is_always_the_first_non_root_row() {
    let mut entries = vec![InspectorEntry {
        name: "marina_color.ytd".to_owned(),
        logical_path: "textures/characters/marina_color.ytd".to_owned(),
        ..InspectorEntry::default()
    }];

    prepend_parent_navigation(&mut entries, "textures/characters", false);

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "../");
    assert_eq!(entries[0].logical_path, "textures");
    assert_eq!(entries[0].marker(), "UP");
    assert!(entries[0].is_parent_navigation());
}

#[test]
fn root_listing_does_not_invent_a_parent_row() {
    let mut entries = vec![InspectorEntry::default()];

    prepend_parent_navigation(&mut entries, "", false);

    assert_eq!(entries.len(), 1);
    assert!(!entries[0].is_parent_navigation());
}

#[test]
fn provider_manifest_parent_returns_to_the_asset_directory() {
    let mut entries = Vec::new();

    prepend_parent_navigation(&mut entries, "textures/characters/marina_color.ytd", true);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "../");
    assert_eq!(entries[0].logical_path, "textures/characters");
}

#[test]
fn activating_parent_row_navigates_up_without_closing_the_open_document() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.current_path = "textures/characters".to_owned();
    runtime.entries = vec![InspectorEntry::parent_navigation("textures")];
    runtime.document = Some(AssetDocument::default());
    runtime.browser_window_start = 0;
    runtime.last_refresh_frame = Some(41);

    runtime.activate_row(0, false, 42);

    assert_eq!(runtime.current_path, "textures");
    assert_eq!(runtime.browser_window_start, 0);
    assert!(runtime.last_refresh_frame.is_none());
    assert!(runtime.document.is_some());
}

#[test]
fn browser_navigation_keeps_open_document_and_preview() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.selected_index = Some(4);
    runtime.document = Some(AssetDocument::default());
    runtime.preview_snapshot = Some(AssetPreviewSnapshot {
        asset_ref: "textures/test.ytd".to_owned(),
        kind: newengine_asset_preview_runtime::AssetPreviewKind::Texture2d,
        ready: true,
        texture_ref: Some("textures/test.ytd@entry".to_owned()),
        ui_texture_id: None,
        width: 64,
        height: 64,
        diagnostic: None,
    });

    runtime.clear_browser_selection();

    assert_eq!(runtime.selected_index, None);
    assert!(runtime.document.is_some());
    assert!(runtime.preview_snapshot.as_ref().is_some_and(|preview| {
        preview.ready
            && preview.kind == newengine_asset_preview_runtime::AssetPreviewKind::Texture2d
    }));
}

#[test]
fn generic_modes_only_depend_on_directory_state() {
    assert!(AssetInspectorMode::Assets.accepts(false));
    assert!(!AssetInspectorMode::Assets.accepts(true));
    assert!(AssetInspectorMode::Folders.accepts(true));
}
