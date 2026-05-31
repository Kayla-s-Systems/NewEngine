pub(crate) fn publish_loading_overlay(
    status: &ScreenOverlayStatus,
    provider: UiProviderBinding,
    frame_index: u64,
) {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        log::warn!("ui gateway: engine.ui route unavailable; loading overlay skipped without native/special renderer");
        return;
    }

    let projection = loading_surface_projection(status, provider);
    // Active retained loading surfaces must not render 100% until they are
    // explicitly hidden by `publish_loading_overlay_inactive`. `SceneLaunchStatus`
    // intentionally uses values like 0.995 during handoff, and {:.0} would round
    // that to 100%, which looked like a completed world while render residency
    // was still pending.
    let progress_percent = (status.progress_01() * 100.0).clamp(0.0, 99.0);
    let progress_01 = status.progress_01().clamp(0.0, 0.995);
    let lines = vec![
        status.title.clone(),
        status.status.clone(),
        status.detail.clone(),
        format!("progress={:.0}%", progress_percent),
    ];
    let mut metrics = BTreeMap::new();
    metrics.insert("surface_projection".to_owned(), serde_json::to_value(&projection).unwrap_or(serde_json::Value::Null));
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    let node = UiSurfaceNode {
        version: 1,
        surface_id: if projection.surface_id().trim().is_empty() {
            UI_SURFACE_ENGINE_LOADING.to_owned()
        } else {
            projection.surface_id().to_owned()
        },
        source: "engine.ui.loading".to_owned(),
        visible: true,
        modal: false,
        z_order: 900,
        title: status.title.clone(),
        subtitle: status.status.clone(),
        body_lines: lines.clone(),
        footer_lines: Vec::new(),
        style_tags: vec!["retained".to_owned(), "centered-loading".to_owned(), "progress-card".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: loading_overlay_components(&status.title, &status.status, &status.detail, progress_01, progress_percent),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::Center,
            min_size_px: [560.0, 260.0],
            max_size_px: [820.0, 380.0],
            row_pitch_px: 28.0,
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics,
    };
    publish_surface_node(&node);
}



fn loading_overlay_components(
    title: &str,
    status: &str,
    detail: &str,
    progress_01: f32,
    progress_percent: f32,
) -> Vec<UiComponentNode> {
    let mut progress = UiComponentNode::row("loading.progress_bar", "")
        .with_value(format!("{:.0}%", progress_percent))
        .tagged("progress")
        .tagged("progress-bar");
    progress.component_id = "progress_bar".to_owned();
    progress.props.insert("progress_01".to_owned(), serde_json::json!(progress_01));
    progress.props.insert("percent".to_owned(), serde_json::json!(progress_percent));

    vec![
        UiComponentNode::text("loading.title", title.to_owned())
            .with_tone(newengine_ui_api::UiNodeTone::Accent)
            .tagged("loading-title"),
        UiComponentNode::text("loading.status", status.to_owned())
            .with_tone(newengine_ui_api::UiNodeTone::Normal)
            .tagged("loading-status"),
        progress,
        UiComponentNode::text("loading.detail", detail.to_owned())
            .with_tone(newengine_ui_api::UiNodeTone::Disabled)
            .tagged("loading-detail"),
    ]
}

/// Clears the retained engine.ui.loading surface in the selected `engine.ui` provider.
///
/// Loading surfaces are retained UI nodes. When the scene launch gate completes,
/// publishing no active overlay is not enough: the provider needs an explicit
/// hidden node so `Loading World 100%` cannot remain over the playable frame.
pub(crate) fn publish_loading_overlay_inactive(frame_index: u64) {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        log::warn!("ui gateway: engine.ui route unavailable; loading overlay clear skipped without native/special renderer");
        return;
    }

    let mut metrics = BTreeMap::new();
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    metrics.insert("reason".to_owned(), serde_json::json!("scene-launch-complete"));
    let node = UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(),
        source: "engine.ui.loading".to_owned(),
        visible: false,
        modal: false,
        z_order: 900,
        title: String::new(),
        subtitle: String::new(),
        body_lines: Vec::new(),
        footer_lines: Vec::new(),
        style_tags: vec!["retained".to_owned(), "hidden".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: Vec::new(),
        message: None,
        style: UiSurfaceStyle::default(),
        admission_policy: Default::default(),
        metrics,
    };
    publish_surface_node(&node);
}
