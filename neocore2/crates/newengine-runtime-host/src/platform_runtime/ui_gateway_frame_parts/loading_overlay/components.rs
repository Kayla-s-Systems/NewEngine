use super::*;
use newengine_core::loading::LoadingVisualRefs;

pub(super) fn loading_overlay_components(
    progress_01: f32,
    progress_percent: f32,
    frame_index: u64,
    visuals: &LoadingVisualRefs,
) -> Vec<UiComponentNode> {
    let mut components = Vec::new();

    if let Some(texture_ref) = valid_visual_ref(visuals.background.as_deref()) {
        let mut background = UiComponentNode::text("loading.background", "")
            .tagged("startup-background")
            .tagged("fill")
            .tagged("stretch");
        background.component_id = newengine_ui_api::UI_COMPONENT_EXTERNAL_TEXTURE.to_owned();
        set_ytd_texture_ref(&mut background, texture_ref);
        set_production_paint_only(&mut background);
        background
            .props
            .insert("fit".to_owned(), serde_json::json!("cover"));
        background
            .props
            .insert("layer".to_owned(), serde_json::json!("background"));
        background
            .props
            .insert("fill".to_owned(), serde_json::json!(true));
        background
            .props
            .insert("position".to_owned(), serde_json::json!("fixed"));
        background
            .props
            .insert("display".to_owned(), serde_json::json!("leaf"));
        background
            .props
            .insert("z_order".to_owned(), serde_json::json!(0));
        components.push(background);
    }

    if let Some(texture_ref) = valid_visual_ref(visuals.logo.as_deref()) {
        let mut logo = UiComponentNode::text("loading.logo", "")
            .tagged("startup-logo")
            .tagged("loading-brand-logo");
        logo.component_id = newengine_ui_api::UI_COMPONENT_EXTERNAL_TEXTURE.to_owned();
        set_ytd_texture_ref(&mut logo, texture_ref);
        set_production_paint_only(&mut logo);
        logo.props
            .insert("position".to_owned(), serde_json::json!("fixed"));
        logo.props
            .insert("display".to_owned(), serde_json::json!("leaf"));
        logo.props
            .insert("anchor".to_owned(), serde_json::json!("center"));
        logo.props
            .insert("w_px".to_owned(), serde_json::json!(360.0));
        logo.props
            .insert("h_px".to_owned(), serde_json::json!(360.0));
        logo.props
            .insert("z_order".to_owned(), serde_json::json!(10));
        components.push(logo);
    }

    if let Some(texture_ref) = valid_visual_ref(visuals.spinner.as_deref()) {
        let mut spinner = UiComponentNode::text("loading.spinner", "")
            .tagged("startup-spinner")
            .tagged("loading-spinner");
        spinner.component_id = newengine_ui_api::UI_COMPONENT_EXTERNAL_TEXTURE.to_owned();
        set_ytd_texture_ref(&mut spinner, texture_ref);
        set_production_paint_only(&mut spinner);
        spinner
            .props
            .insert("position".to_owned(), serde_json::json!("fixed"));
        spinner
            .props
            .insert("display".to_owned(), serde_json::json!("leaf"));
        spinner
            .props
            .insert("anchor".to_owned(), serde_json::json!("bottom_center"));
        spinner
            .props
            .insert("bottom_px".to_owned(), serde_json::json!(96.0));
        spinner
            .props
            .insert("w_px".to_owned(), serde_json::json!(64.0));
        spinner
            .props
            .insert("h_px".to_owned(), serde_json::json!(64.0));
        spinner
            .props
            .insert("z_order".to_owned(), serde_json::json!(20));
        spinner
            .props
            .insert("animation".to_owned(), serde_json::json!("rotate"));
        spinner
            .props
            .insert("frame_index".to_owned(), serde_json::json!(frame_index));
        components.push(spinner);
    }

    let mut progress = UiComponentNode::row("loading.progress_bar", "")
        .with_value("")
        .tagged("progress")
        .tagged("progress-bar");
    progress.component_id = "progress_bar".to_owned();
    progress
        .props
        .insert("position".to_owned(), serde_json::json!("fixed"));
    progress
        .props
        .insert("anchor".to_owned(), serde_json::json!("bottom_center"));
    progress
        .props
        .insert("bottom_px".to_owned(), serde_json::json!(48.0));
    progress
        .props
        .insert("w_px".to_owned(), serde_json::json!(760.0));
    progress
        .props
        .insert("h_px".to_owned(), serde_json::json!(18.0));
    progress
        .props
        .insert("debug_chrome".to_owned(), serde_json::json!(false));
    progress
        .props
        .insert("label_visible".to_owned(), serde_json::json!(false));
    progress
        .props
        .insert("progress_01".to_owned(), serde_json::json!(progress_01));
    progress
        .props
        .insert("percent".to_owned(), serde_json::json!(progress_percent));
    progress
        .props
        .insert("z_order".to_owned(), serde_json::json!(30));
    components.push(progress);

    components
}

fn set_production_paint_only(component: &mut UiComponentNode) {
    component
        .props
        .insert("paint_only".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("debug_chrome".to_owned(), serde_json::json!(false));
    component
        .props
        .insert("hit_test".to_owned(), serde_json::json!(false));
}

fn valid_visual_ref(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn set_ytd_texture_ref(component: &mut UiComponentNode, texture_ref: &str) {
    component
        .props
        .insert("texture_ref".to_owned(), serde_json::json!(texture_ref));
    // Compatibility with authored .neui Image/ExternalTexture nodes: Aurelia
    // consumes either texture_ref or texture, but both must be runtime refs.
    component
        .props
        .insert("texture".to_owned(), serde_json::json!(texture_ref));
}

pub(super) fn error_overlay_components(status: &ScreenOverlayStatus) -> Vec<UiComponentNode> {
    let mut reason = UiComponentNode::row("error.reason", "Reason")
        .with_value(format!("{:?}", status.reason))
        .tagged("error-reason")
        .tagged("diagnostic");
    reason.component_id = "status_badge".to_owned();

    let mut detail = UiComponentNode::text("error.detail", status.detail.clone())
        .with_tone(newengine_ui_api::UiNodeTone::Disabled)
        .tagged("error-detail")
        .tagged("diagnostic-body");
    detail
        .props
        .insert("selectable".to_owned(), serde_json::json!(true));

    vec![
        UiComponentNode::text("error.title", status.title.clone())
            .with_tone(newengine_ui_api::UiNodeTone::Danger)
            .tagged("error-title"),
        UiComponentNode::text("error.status", status.status.clone())
            .with_tone(newengine_ui_api::UiNodeTone::Accent)
            .tagged("error-status"),
        reason,
        detail,
        UiComponentNode::text(
            "error.footer",
            "NORTHSTAR // renderer failure captured; process held for diagnostics.".to_owned(),
        )
        .with_tone(newengine_ui_api::UiNodeTone::Disabled)
        .tagged("error-footer"),
    ]
}
