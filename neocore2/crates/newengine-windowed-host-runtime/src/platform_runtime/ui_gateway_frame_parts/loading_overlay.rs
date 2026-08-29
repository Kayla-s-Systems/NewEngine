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
    let node = build_overlay_surface_node(status, provider, frame_index);
    publish_surface_node(&node);
}

pub(crate) fn publish_loading_overlay_inactive(frame_index: u64) {
    // The host-side loading texture cache only proves that a payload was emitted
    // during the current retained-surface session. Tear it down with the surface so
    // a later loading session or renderer reload cannot reuse stale external IDs.
    super::draw_list::reset_loading_texture_session();

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
    match ui_unmount_surface_call().call_optional(&payload) {
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
