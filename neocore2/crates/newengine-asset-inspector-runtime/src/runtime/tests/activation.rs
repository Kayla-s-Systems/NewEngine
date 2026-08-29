use super::super::*;

#[test]
fn click_on_already_selected_open_asset_schedules_refresh() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![InspectorEntry {
        name: "axe.ydd".to_owned(),
        logical_path: "models/axe.ydd".to_owned(),
        ..InspectorEntry::default()
    }];
    runtime.selected_index = Some(0);
    runtime.document = Some(AssetDocument {
        asset_ref: "models/axe.ydd".to_owned(),
        ..Default::default()
    });

    runtime.activate_row(0, false, 42);

    let pending = runtime
        .pending_entry_activation
        .as_ref()
        .expect("second click on the selected file must reopen its preview");
    assert_eq!(pending.entry.logical_path, "models/axe.ydd");
}

#[test]
fn second_click_on_selected_file_always_schedules_open() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![InspectorEntry {
        name: "tool_axe.nemat".to_owned(),
        logical_path: "materials/tool_axe.nemat".to_owned(),
        ..InspectorEntry::default()
    }];

    runtime.activate_row(0, false, 10);
    assert!(runtime.pending_entry_activation.is_none());
    runtime.activate_row(0, false, 5000);

    let pending = runtime.pending_entry_activation.as_ref().unwrap();
    assert_eq!(pending.entry.logical_path, "materials/tool_axe.nemat");
    assert_eq!(pending.absolute_index, 0);
}

#[test]
fn second_click_on_different_file_only_changes_selection() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![
        InspectorEntry {
            name: "a.ydd".to_owned(),
            logical_path: "models/a.ydd".to_owned(),
            ..InspectorEntry::default()
        },
        InspectorEntry {
            name: "b.ytd".to_owned(),
            logical_path: "textures/b.ytd".to_owned(),
            ..InspectorEntry::default()
        },
    ];

    runtime.activate_row(0, false, 10);
    runtime.activate_row(1, false, 11);

    assert_eq!(runtime.selected_index, Some(1));
    assert!(runtime.pending_entry_activation.is_none());
}

#[test]
fn explicit_double_click_opens_unselected_file_immediately() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![InspectorEntry {
        name: "asset_layout.json".to_owned(),
        logical_path: "asset_layout.json".to_owned(),
        ..InspectorEntry::default()
    }];

    runtime.activate_row(0, true, 10);

    assert!(runtime.pending_entry_activation.is_some());
}

#[test]
fn single_click_selects_file_without_starting_provider_decode() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![InspectorEntry {
        name: "material.json".to_owned(),
        logical_path: "materials/material.json".to_owned(),
        ..InspectorEntry::default()
    }];

    runtime.activate_row(0, false, 42);

    assert_eq!(runtime.selected_index, Some(0));
    assert!(runtime.pending_entry_activation.is_none());
    assert!(runtime.activity.is_none());
    assert!(runtime.status.contains("click again to open preview"));
}

#[test]
fn file_activation_is_deferred_one_frame_so_progress_can_be_presented() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![InspectorEntry {
        name: "marina_color.ytd".to_owned(),
        logical_path: "textures/characters/marina_color.ytd".to_owned(),
        ..InspectorEntry::default()
    }];

    runtime.activate_row(0, true, 42);

    let pending = runtime
        .pending_entry_activation
        .as_ref()
        .expect("file activation must be deferred");
    assert_eq!(pending.absolute_index, 0);
    assert_eq!(pending.requested_frame, 42);
    assert_eq!(runtime.selected_index, Some(0));
    assert!(runtime.activity.is_some());
    assert!(runtime.document.is_none());
}
