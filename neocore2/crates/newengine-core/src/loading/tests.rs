use crate::loading::{
    BootDrawCommand, BootFrameDto, BootViewport, EngineLoadingKernel, LoadingPhase, LoadingProfile,
    LoadingVisualRole, ResolvedLoadingAssignment, ENGINE_LOADING_PLUGIN_ID,
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
        profile.visuals.primary_logo(),
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

#[test]
fn phase_specific_visuals_do_not_reuse_prestart_when_runtime_is_authored() {
    let mut startup = crate::startup::StartupConfig::default();
    startup.plugins.insert(
        ENGINE_LOADING_PLUGIN_ID.to_owned(),
        serde_json::json!({
            "manifest_id": "app.phase-aware.loading",
            "prestart": {
                "logo": "consumer.loading.prestart"
            },
            "runtime_loading": {
                "background": "consumer.loading.runtime.bg",
                "logos": [
                    "consumer.loading.runtime.primary",
                    {"texture_ref": "consumer.loading.runtime.partner"},
                    "consumer.loading.runtime.primary",
                    ""
                ],
                "spinner": "consumer.loading.runtime.spinner"
            },
            "world_handoff": {
                "logo": "consumer.loading.world"
            }
        }),
    );

    let profile = LoadingProfile::from_startup_config(&startup);
    assert_eq!(
        profile
            .visuals_for_phase(LoadingPhase::PreStart)
            .logo_refs(),
        vec!["consumer.loading.prestart"]
    );
    assert_eq!(
        profile
            .visuals_for_phase(LoadingPhase::RuntimeLoading)
            .logo_refs(),
        vec![
            "consumer.loading.runtime.primary",
            "consumer.loading.runtime.partner"
        ]
    );
    assert_eq!(
        profile
            .visuals_for_phase(LoadingPhase::WorldHandoff)
            .logo_refs(),
        vec!["consumer.loading.world"]
    );

    let runtime = ResolvedLoadingAssignment::from_profile(LoadingPhase::RuntimeLoading, &profile);
    assert_eq!(runtime.visuals.image_layer_count(), 4);
    assert_eq!(
        runtime.visuals.background.as_deref(),
        Some("consumer.loading.runtime.bg")
    );
}

#[test]
fn boot_frame_emits_multiple_logos_in_manifest_order() {
    let mut startup = crate::startup::StartupConfig::default();
    startup.plugins.insert(
        ENGINE_LOADING_PLUGIN_ID.to_owned(),
        serde_json::json!({
            "runtime_loading": {
                "logos": [
                    "consumer.loading.logo-a",
                    "consumer.loading.logo-b",
                    "consumer.loading.logo-c"
                ]
            }
        }),
    );

    let profile = LoadingProfile::from_startup_config(&startup);
    let assignment =
        ResolvedLoadingAssignment::from_profile(LoadingPhase::RuntimeLoading, &profile);
    let frame = BootFrameDto::from_status(
        assignment,
        BootViewport::default(),
        "Title",
        "Status",
        "Detail",
        0.5,
        3,
    );

    let logos: Vec<_> = frame
        .commands
        .iter()
        .filter_map(|command| match command {
            BootDrawCommand::Image {
                role: LoadingVisualRole::Logo,
                texture_ref,
                rect,
                ..
            } => Some((texture_ref.as_str(), *rect)),
            _ => None,
        })
        .collect();

    assert_eq!(
        logos
            .iter()
            .map(|(texture_ref, _)| *texture_ref)
            .collect::<Vec<_>>(),
        vec![
            "consumer.loading.logo-a",
            "consumer.loading.logo-b",
            "consumer.loading.logo-c"
        ]
    );
    assert!(logos.windows(2).all(|pair| pair[0].1 != pair[1].1));
}

#[test]
fn legacy_singular_logo_remains_supported() {
    let value = serde_json::json!({
        "background": null,
        "logo": "legacy.logo",
        "spinner": null,
        "source": "legacy"
    });
    let visuals: crate::loading::LoadingVisualRefs = serde_json::from_value(value).unwrap();

    assert_eq!(visuals.logo_refs(), vec!["legacy.logo"]);
    assert_eq!(visuals.image_layer_count(), 1);
}
