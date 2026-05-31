pub(crate) fn publish_surface_node(node: &UiSurfaceNode) {
    let payload = match serde_json::to_vec(node) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("ui gateway: failed to encode surface node surface='{}': {e}", node.surface_id);
            return;
        }
    };
    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_SURFACE_NODE_V1,
        &payload,
    ) {
        Ok(Some(_)) => {}
        Ok(None) => log::warn!(
            "ui gateway: engine.ui route unavailable; surface='{}' skipped without native/special renderer",
            node.surface_id,
        ),
        Err(e) => log::warn!("ui gateway: surface node publish failed surface='{}' err='{e}'", node.surface_id),
    }
}


pub(crate) fn publish_node_tree_request(request: &UiNodeTreeRequest) {
    let payload = match serde_json::to_vec(request) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!(
                "ui gateway: failed to encode node tree request surface='{}': {e}",
                request.surface_id
            );
            return;
        }
    };
    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => {
            if let Ok(ack) = serde_json::from_slice::<UiNodeRequestAck>(&bytes) {
                log::trace!(
                    "ui gateway: node tree request accepted surface='{}' nodes={} provider={}",
                    ack.surface_id,
                    ack.accepted_nodes,
                    ack.provider.as_deref().unwrap_or("engine.ui")
                );
            }
        }
        Ok(None) => log::warn!(
            "ui gateway: engine.ui route unavailable; node tree surface='{}' skipped",
            request.surface_id,
        ),
        Err(e) => log::warn!(
            "ui gateway: node tree publish failed surface='{}' err='{e}'",
            request.surface_id
        ),
    }
}

/// Publishes provider-neutral runtime debug telemetry through `engine.ui`.
///
/// This lives in runtime-host, not render-controller: render produces telemetry
/// resources, while the host owns service routing to UI providers.
pub(crate) fn publish_debug_overlay_telemetry(telemetry: &UiRuntimeDebugOverlayTelemetry) {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        return;
    }
    let mut lines = if telemetry.lines.is_empty() {
        telemetry.text.lines().map(str::to_owned).collect::<Vec<_>>()
    } else {
        telemetry.lines.clone()
    };
    if lines.is_empty() {
        lines.push(format!("frame={} source={}", telemetry.frame_index, telemetry.source));
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
        z_order: -10_000,
        title: "RUNTIME DEBUG".to_owned(),
        subtitle: telemetry.source.clone(),
        body_lines: lines.clone(),
        footer_lines: vec!["Runtime Debug is a bottom-layer surface; other UI may cover it.".to_owned()],
        style_tags: vec!["retained".to_owned(), "runtime-debug".to_owned(), "bottom-layer".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: std::iter::once(
            UiComponentNode::action("debug.toggle", "Show/Hide", "runtime.debug.toggle")
                .with_detail("Toggle Runtime Debug visibility")
                .with_tone(UiNodeTone::Accent)
                .tagged("debug-toggle"),
        )
        .chain(
            lines
                .iter()
                .enumerate()
                .map(|(index, line)| UiComponentNode::text(format!("debug.line.{index}"), line.clone())),
        )
        .collect(),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::BottomLeft,
            min_size_px: [360.0, 180.0],
            max_size_px: [620.0, 520.0],
            margin_px: [12.0, 12.0],
            row_pitch_px: 22.0,
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics: telemetry.metrics.clone(),
    };
    publish_surface_node(&node);
}


fn ui_draw_list_stats(draw_list: &UiDrawList) -> String {
    let texture_set_bytes: usize = draw_list
        .texture_delta
        .set
        .values()
        .map(|texture| texture.rgba8.len())
        .sum();
    let patch_bytes: usize = draw_list
        .texture_delta
        .patches
        .iter()
        .map(|patch| patch.rgba8.len())
        .sum();
    format!(
        "ui(vertices={} indices={} cmds={} tex_set={} tex_set_bytes={} patches={} patch_bytes={} free={})",
        draw_list.mesh.vertices.len(),
        draw_list.mesh.indices.len(),
        draw_list.mesh.cmds.len(),
        draw_list.texture_delta.set.len(),
        texture_set_bytes,
        draw_list.texture_delta.patches.len(),
        patch_bytes,
        draw_list.texture_delta.free.len(),
    )
}
