use super::super::*;

#[test]
fn preview_capture_uses_provider_dispatch_owner() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.preview_snapshot = Some(AssetPreviewSnapshot {
        asset_ref: "models/test.ydd".to_owned(),
        kind: AssetPreviewKind::Scene3d,
        ready: true,
        texture_ref: None,
        ui_texture_id: Some(0x8000_0001),
        width: 488,
        height: 236,
        diagnostic: None,
    });
    let dispatch = UiEventDispatchFrame {
        capture_state: newengine_ui_api::UiPointerCaptureState {
            active: true,
            owner_surface_id: ASSET_INSPECTOR_SURFACE_ID.to_owned(),
            owner_node_id: "asset.inspector.preview.image".to_owned(),
            button: Some(0),
            reason: "pointer press".to_owned(),
        },
        ..UiEventDispatchFrame::default()
    };

    runtime.handle_preview_camera_input(&dispatch, Some(&UiInputFrame::default()));

    assert!(runtime.preview_pointer_captured);
    assert!(runtime.status.contains("mouse captured"));
}

#[test]
fn middle_mouse_drag_activates_preview_camera_pan() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.preview_snapshot = Some(AssetPreviewSnapshot {
        asset_ref: "models/test.ydd".to_owned(),
        kind: AssetPreviewKind::Scene3d,
        ready: true,
        texture_ref: None,
        ui_texture_id: Some(0x8000_0001),
        width: 488,
        height: 236,
        diagnostic: None,
    });
    let dispatch = UiEventDispatchFrame {
        hovered_node: Some(newengine_ui_api::UiHitTestResult {
            surface_id: ASSET_INSPECTOR_SURFACE_ID.to_owned(),
            node_id: "asset.inspector.preview.image".to_owned(),
            ..Default::default()
        }),
        ..UiEventDispatchFrame::default()
    };
    let mut input = UiInputFrame::default();
    input.mouse_down.insert(PREVIEW_PAN_MOUSE_BUTTON);
    input.mouse_delta = (12.0, -6.0);

    runtime.handle_preview_camera_input(&dispatch, Some(&input));

    assert!(runtime.preview_middle_pan_active);
    assert!(runtime.preview_pointer_captured);
    assert!(runtime.status.contains("MMB camera pan active"));
}

#[test]
fn pending_preview_open_is_not_executed_in_request_frame() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.entries = vec![InspectorEntry {
        name: "model.ydd".to_owned(),
        logical_path: "models/model.ydd".to_owned(),
        ..InspectorEntry::default()
    }];
    runtime.activate_row(0, true, 42);
    runtime.execute_pending_entry_activation(42);
    assert!(runtime.pending_entry_activation.is_some());
    assert!(runtime.document.is_none());
}

#[test]
fn newest_preview_request_replaces_stale_pending_open() {
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
            name: "b.ydd".to_owned(),
            logical_path: "models/b.ydd".to_owned(),
            ..InspectorEntry::default()
        },
    ];
    runtime.activate_row(0, true, 10);
    runtime.activate_row(1, true, 11);
    assert_eq!(
        runtime
            .pending_entry_activation
            .as_ref()
            .unwrap()
            .entry
            .logical_path,
        "models/b.ydd"
    );
}

#[test]
fn preview_entries_scrolling_selects_absolute_entry() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.preview_entries = (0..12)
        .map(|index| InspectorEntry {
            name: format!("entry-{index}"),
            logical_path: format!("materials/test.nemat@entry-{index}"),
            ..InspectorEntry::default()
        })
        .collect();
    runtime.preview_entries_window_start = 5;
    runtime.activate_preview_entry(2, 100);
    assert_eq!(runtime.selected_preview_entry, Some(7));
    assert_eq!(
        runtime
            .pending_preview_entry_activation
            .as_ref()
            .unwrap()
            .entry
            .logical_path,
        "materials/test.nemat@entry-7"
    );
}

#[test]
fn preview_entry_cache_is_bounded_and_promotes_hits() {
    let mut cache = PreviewEntryCache::default();
    for index in 0..(PREVIEW_ENTRY_CACHE_CAPACITY + 2) {
        cache.insert(
            &format!("materials/{index}.nemat"),
            &[InspectorEntry {
                name: format!("entry-{index}"),
                logical_path: format!("materials/{index}.nemat@entry"),
                ..InspectorEntry::default()
            }],
        );
    }
    assert_eq!(cache.entries.len(), PREVIEW_ENTRY_CACHE_CAPACITY);
    assert!(cache.get("materials/0.nemat").is_none());
    assert!(cache.get("materials/9.nemat").is_some());
    assert_eq!(
        cache.entries.most_recent_key().unwrap().as_str(),
        "materials/9.nemat"
    );
}

#[test]
fn info_modal_actions_toggle_visibility_without_provider_work() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.document = Some(AssetDocument {
        asset_ref: "materials/test.nemat".to_owned(),
        ..AssetDocument::default()
    });
    runtime.handle_actions(&UiEventDispatchFrame {
        frame_index: 10,
        actions: vec![newengine_ui_api::UiActionDispatch {
            surface_id: ASSET_INSPECTOR_SURFACE_ID.to_owned(),
            node_id: "asset.inspector.info.open".to_owned(),
            action_id: ACTION_INFO_OPEN.to_owned(),
            trigger: UiNodeEventTrigger::Click,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert!(runtime.info_modal_visible);

    runtime.handle_actions(&UiEventDispatchFrame {
        frame_index: 11,
        actions: vec![newengine_ui_api::UiActionDispatch {
            surface_id: ASSET_INSPECTOR_SURFACE_ID.to_owned(),
            node_id: "asset.inspector.info.close".to_owned(),
            action_id: ACTION_INFO_CLOSE.to_owned(),
            trigger: UiNodeEventTrigger::Click,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert!(!runtime.info_modal_visible);
}
