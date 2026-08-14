use super::*;

#[path = "loading_overlay/components.rs"]
mod components;
#[path = "loading_overlay/surface_node.rs"]
mod surface_node;

use self::surface_node::{build_overlay_surface_node, hidden_loading_overlay_node};

pub(crate) fn publish_loading_overlay(
    status: &ScreenOverlayStatus,
    provider: UiProviderBinding,
    frame_index: u64,
) {
    if !engine_ui_route_available("loading/error overlay") {
        return;
    }

    let node = build_overlay_surface_node(status, provider, frame_index);
    publish_surface_node(&node);
}

pub(crate) fn publish_loading_overlay_inactive(frame_index: u64) {
    // The host-side loading texture cache only proves that a payload was emitted
    // during the current retained-surface session. Tear it down with the surface so
    // a later loading session or renderer reload cannot reuse stale external IDs.
    super::draw_list::reset_loading_texture_session();

    if !engine_ui_route_available("loading overlay clear") {
        return;
    }

    // First publish a hidden node so providers that retain visibility state can
    // invalidate focus/pointer capture. Then unmount the retained loading surface
    // entirely: a stale fullscreen loading node must never remain in hit-testing
    // after the editor/game launch gate has been released.
    let node = hidden_loading_overlay_node(frame_index);
    publish_surface_node(&node);

    let request = UiSurfaceRequest {
        surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(),
    };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: failed to encode loading surface unmount request err='{}'",
                error
            );
            return;
        }
    };
    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1,
        &payload,
    ) {
        Ok(Some(_)) => newengine_ulog_api::ulog::info!(
            "ui gateway: loading surface unmounted after launch frame={}",
            frame_index
        ),
        Ok(None) => newengine_ulog_api::ulog::warn!(
            "ui gateway: engine.ui route unavailable while unmounting loading surface"
        ),
        Err(error) => newengine_ulog_api::ulog::warn!(
            "ui gateway: loading surface unmount failed err='{}'",
            error
        ),
    }
}

fn engine_ui_route_available(operation: &str) -> bool {
    if newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        return true;
    }

    newengine_ulog_api::ulog::warn!(
        "ui gateway: engine.ui route unavailable; {operation} skipped without native/special renderer"
    );
    false
}
