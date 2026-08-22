use super::*;
pub(crate) fn publish_surface_node(node: &UiSurfaceNode) {
    let payload = match serde_json::to_vec(node) {
        Ok(payload) => payload,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: failed to encode surface node surface='{}': {e}",
                node.surface_id
            );
            return;
        }
    };
    match ui_surface_node_call().call_optional(&payload) {
        Ok(Some(_)) => {}
        Ok(None) => newengine_ulog_api::ulog::warn!(
            "ui gateway: engine.ui route unavailable; surface='{}' skipped without native/special renderer",
            node.surface_id,
        ),
        Err(e) => newengine_ulog_api::ulog::warn!("ui gateway: surface node publish failed surface='{}' err='{e}'", node.surface_id),
    }
}

pub(crate) fn set_surface_visible(surface_id: &str, visible: bool) -> bool {
    let surface_id = surface_id.trim();
    if surface_id.is_empty() {
        return false;
    }
    let request = UiSurfaceVisibilityRequest {
        surface_id: surface_id.to_owned(),
        visible,
    };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: failed to encode surface visibility surface='{}' err='{}'",
                surface_id,
                error
            );
            return false;
        }
    };
    match ui_set_surface_visible_call().call_optional(&payload) {
        Ok(Some(_)) => true,
        Ok(None) => {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: engine.ui route unavailable; visibility surface='{}' skipped",
                surface_id
            );
            false
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: surface visibility failed surface='{}' visible={} err='{}'",
                surface_id,
                visible,
                error
            );
            false
        }
    }
}

pub(crate) fn publish_node_tree_request(request: &UiNodeTreeRequest) {
    let payload = match serde_json::to_vec(request) {
        Ok(payload) => payload,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: failed to encode node tree request surface='{}': {e}",
                request.surface_id
            );
            return;
        }
    };
    match ui_apply_node_request_call().call_optional(&payload) {
        Ok(Some(_)) => {}
        Ok(None) => newengine_ulog_api::ulog::warn!(
            "ui gateway: engine.ui route unavailable; node tree surface='{}' skipped",
            request.surface_id,
        ),
        Err(e) => newengine_ulog_api::ulog::warn!(
            "ui gateway: node tree publish failed surface='{}' err='{e}'",
            request.surface_id
        ),
    }
}

/// Publishes provider-neutral runtime debug telemetry through `engine.ui`.
///
/// This lives in runtime-host, not render-controller: render produces telemetry
/// resources, while the host owns service routing to UI providers.
fn debug_overlay_surface_node(telemetry: &UiRuntimeDebugOverlayTelemetry) -> UiSurfaceNode {
    let mut lines = if telemetry.lines.is_empty() {
        telemetry
            .text
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    } else {
        telemetry.lines.clone()
    };
    if lines.is_empty() {
        lines.push(format!(
            "frame={} source={}",
            telemetry.frame_index, telemetry.source
        ));
    }
    let node = UiSurfaceNode {
        version: 1,
        surface_id: if telemetry.surface_id.trim().is_empty() {
            UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned()
        } else {
            telemetry.surface_id.clone()
        },
        source: telemetry.source.clone(),
        visible: true,
        modal: false,
        // Above ordinary runtime/game surfaces but below loading/error overlays.
        z_order: 800,
        title: String::new(),
        subtitle: String::new(),
        body_lines: lines.clone(),
        footer_lines: Vec::new(),
        style_tags: vec![
            "retained".to_owned(),
            "runtime-debug".to_owned(),
            "technical-overlay".to_owned(),
            "bottom-right".to_owned(),
        ],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: lines
            .iter()
            .enumerate()
            .map(|(index, line)| UiComponentNode::text(format!("debug.line.{index}"), line.clone()))
            .collect(),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::BottomRight,
            min_size_px: [360.0, 88.0],
            max_size_px: [640.0, 300.0],
            margin_px: [12.0, 12.0],
            padding_px: [10.0, 10.0, 10.0, 10.0],
            row_pitch_px: 18.0,
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics: telemetry.metrics.clone(),
    };
    node
}

pub(crate) fn publish_debug_overlay_telemetry(telemetry: &UiRuntimeDebugOverlayTelemetry) {
    let node = debug_overlay_surface_node(telemetry);
    publish_surface_node(&node);
}

#[cfg(test)]
mod technical_overlay_tests {
    use super::*;

    #[test]
    fn technical_overlay_is_bottom_right_and_chrome_free() {
        let telemetry = UiRuntimeDebugOverlayTelemetry::new(
            42,
            "FPS 60.0 | TRI 1234 | DRAWS 12
RG pass 8/0 | cpu 1.20ms | gpu 0.80ms",
        );
        let node = debug_overlay_surface_node(&telemetry);
        assert_eq!(node.surface_id, UI_SURFACE_RUNTIME_DEBUG_OVERLAY);
        assert_eq!(node.style.anchor, UiSurfaceAnchor::BottomRight);
        assert_eq!(node.z_order, 800);
        assert!(node.title.is_empty());
        assert!(node.subtitle.is_empty());
        assert!(node.footer_lines.is_empty());
        assert_eq!(node.components.len(), 2);
        assert!(node.style_tags.iter().any(|tag| tag == "technical-overlay"));
        assert!(node.style_tags.iter().any(|tag| tag == "bottom-right"));
    }
}
