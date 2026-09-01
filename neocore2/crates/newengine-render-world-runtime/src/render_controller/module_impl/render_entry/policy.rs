use super::*;

pub(super) fn resolve_viewport_clear_color(
    external_preview_target: bool,
    scene_clear_color: Option<[f32; 4]>,
    configured_clear_color: [f32; 4],
) -> [f32; 4] {
    if external_preview_target {
        ASSET_PREVIEW_EDITOR_CLEAR_COLOR
    } else {
        scene_clear_color.unwrap_or(configured_clear_color)
    }
}

pub(super) fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        (*msg).to_owned()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

pub(super) fn runtime_debug_overlay_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let configured = newengine_runtime_env::var("NEWENGINE_RUNTIME_DEBUG_OVERLAY");
        parse_runtime_debug_overlay_setting(configured.as_deref())
    })
}

pub(super) fn parse_runtime_debug_overlay_setting(value: Option<&str>) -> bool {
    match value.map(str::trim).filter(|it| !it.is_empty()) {
        // Default game viewport should be a clean HUD-only surface. Enable this
        // explicitly with NEWENGINE_RUNTIME_DEBUG_OVERLAY=1 when diagnosing frame
        // metrics; otherwise the retained debug surface churns the UI every frame.
        None => false,
        Some("0") | Some("false") | Some("FALSE") | Some("False") | Some("no") | Some("NO")
        | Some("No") | Some("off") | Some("OFF") | Some("Off") => false,
        Some("1") | Some("true") | Some("TRUE") | Some("True") | Some("yes") | Some("YES")
        | Some("Yes") | Some("on") | Some("ON") | Some("On") => true,
        Some(_) => true,
    }
}
