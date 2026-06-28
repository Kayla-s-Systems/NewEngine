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
    if !engine_ui_route_available("loading overlay clear") {
        return;
    }

    let node = hidden_loading_overlay_node(frame_index);
    publish_surface_node(&node);
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
