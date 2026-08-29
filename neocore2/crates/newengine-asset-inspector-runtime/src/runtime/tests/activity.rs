use super::super::activity::inspector_activity_progress_01;
use super::super::*;

#[test]
fn activity_progress_advances_and_stops_below_completion_while_running() {
    let activity = InspectorActivity {
        label: "OPENING".to_owned(),
        started_frame: 10,
        completed_frame: None,
        waiting_for_preview: false,
        last_published_frame: 10,
    };

    let start = inspector_activity_progress_01(&activity, 10);
    let later = inspector_activity_progress_01(&activity, 70);

    assert!(later > start);
    assert!(later <= 0.90);
}

#[test]
fn completed_activity_animates_to_one() {
    let activity = InspectorActivity {
        label: "OPENING".to_owned(),
        started_frame: 10,
        completed_frame: Some(12),
        waiting_for_preview: false,
        last_published_frame: 12,
    };

    assert!(inspector_activity_progress_01(&activity, 12) < 1.0);
    assert_eq!(
        inspector_activity_progress_01(&activity, 12 + ACTIVITY_COMPLETE_ANIMATION_FRAMES,),
        1.0
    );
}

#[test]
fn activity_publication_is_throttled_to_reduce_full_ui_state_patches() {
    let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
        newengine_viewport_bridge::ViewportBridge::new(),
    )));
    let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
    runtime.begin_activity("OPENING", 10);
    runtime.dirty = false;

    runtime.tick_activity(11);
    assert!(!runtime.dirty);
    runtime.tick_activity(12);
    assert!(!runtime.dirty);

    runtime.tick_activity(13);
    assert!(runtime.dirty);
}
