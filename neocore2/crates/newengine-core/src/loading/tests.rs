use crate::loading::{
    BootFrameDto, BootViewport, EngineLoadingKernel, LoadingPhase, LoadingProfile,
    ResolvedLoadingAssignment, ENGINE_LOADING_PLUGIN_ID,
};
use newengine_math::collections_prelude::NeHashMap;

#[test]
fn prestart_boot_frame_uses_engine_loading_assignment() {
    let mut kernel = EngineLoadingKernel::new();
    let assignment = kernel.resolve_assignment(LoadingPhase::PreStart);
    let frame = kernel.boot_frame(BootViewport::default());

    assert_eq!(assignment.phase, LoadingPhase::PreStart);
    assert_eq!(frame.assignment.phase, LoadingPhase::PreStart);
    assert!(!frame.commands.is_empty());
    assert_eq!(frame.assignment.presenter, "engine.platform.boot_presenter");
    assert_eq!(frame.assignment.visuals.image_layer_count(), 0);
}

#[test]
fn runtime_boot_frame_can_project_status_without_ui() {
    let assignment = ResolvedLoadingAssignment::engine_default(LoadingPhase::RuntimeLoading);
    let frame = BootFrameDto::from_status(
        assignment,
        BootViewport::default(),
        "Title",
        "Status",
        "Detail",
        0.42,
        7,
    );

    assert_eq!(frame.assignment.phase, LoadingPhase::RuntimeLoading);
    assert_eq!(frame.progress.progress_01, 0.42);
    assert_eq!(frame.progress.spinner_phase, 7);
}

#[test]
fn consumer_engine_loading_plugin_assigns_prestart_visuals() {
    let mut startup = crate::startup::StartupConfig::default();
    let mut plugins = NeHashMap::default();
    plugins.insert(
        ENGINE_LOADING_PLUGIN_ID.to_owned(),
        serde_json::json!({
            "manifest_id": "app.gamefps.loading",
            "brand_id": "brand.gamefps",
            "display_name": "GAME FPS",
            "prestart": {
                "background": "consumer.loading.bg",
                "logo": "consumer.loading.logo",
                "spinner": "consumer.loading.spinner"
            }
        }),
    );
    startup.plugins = plugins;

    let profile = LoadingProfile::from_startup_config(&startup);
    assert_eq!(profile.manifest_id, "app.gamefps.loading");
    assert_eq!(
        profile.visuals.logo.as_deref(),
        Some("consumer.loading.logo")
    );

    let mut kernel = EngineLoadingKernel::with_startup_config(&startup);
    let assignment = kernel.resolve_assignment(LoadingPhase::PreStart);
    assert_eq!(assignment.source, "consumer:plugins.engine.loading");
    assert_eq!(assignment.selected, vec!["brand.gamefps".to_owned()]);
    assert_eq!(assignment.visuals.image_layer_count(), 3);
}

#[test]
fn boot_frame_sanitizes_zero_tiny_and_non_finite_viewports() {
    let viewports = [
        BootViewport {
            width: 0.0,
            height: 0.0,
            scale: 0.0,
        },
        BootViewport {
            width: 1.0,
            height: 1.0,
            scale: 1.0,
        },
        BootViewport {
            width: f32::NAN,
            height: f32::INFINITY,
            scale: f32::NEG_INFINITY,
        },
    ];

    for viewport in viewports {
        let frame = BootFrameDto::from_status(
            ResolvedLoadingAssignment::engine_default(LoadingPhase::RuntimeLoading),
            viewport,
            "Title",
            "Status",
            "Detail",
            0.5,
            1,
        );

        assert!(frame.viewport.width.is_finite() && frame.viewport.width > 0.0);
        assert!(frame.viewport.height.is_finite() && frame.viewport.height > 0.0);
        assert!(frame.viewport.scale.is_finite() && frame.viewport.scale > 0.0);

        for command in &frame.commands {
            match command {
                crate::loading::BootDrawCommand::Rect { rect, .. }
                | crate::loading::BootDrawCommand::Image { rect, .. } => {
                    assert!(rect.x.is_finite());
                    assert!(rect.y.is_finite());
                    assert!(rect.w.is_finite() && rect.w >= 0.0);
                    assert!(rect.h.is_finite() && rect.h >= 0.0);
                }
                crate::loading::BootDrawCommand::Text { run } => {
                    assert!(run.x.is_finite());
                    assert!(run.y.is_finite());
                    assert!(run.size_px.is_finite() && run.size_px > 0.0);
                }
                crate::loading::BootDrawCommand::Clear { .. } => {}
            }
        }
    }
}
