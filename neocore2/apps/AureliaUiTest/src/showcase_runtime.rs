use newengine_ui_api::UiNodeRequest;

#[derive(Clone, Debug)]
pub struct AureliaUiTestRouteStatus {
    pub route_available: bool,
    pub ui_backend_capability: bool,
    pub binary_frame_required: bool,
    pub assets_ui_available: bool,
}

impl AureliaUiTestRouteStatus {
    #[inline]
    pub fn new(route_available: bool, ui_backend_capability: bool, binary_frame_required: bool, assets_ui_available: bool) -> Self {
        Self { route_available, ui_backend_capability, binary_frame_required, assets_ui_available }
    }

    #[inline]
    pub fn frame_mode_text(&self) -> &'static str {
        if self.binary_frame_required { "binary strict" } else { "binary + fallback" }
    }
}

pub(crate) fn patch_runtime_values(root: &mut UiNodeRequest, frame_index: u64, click_count: u64, route_status: &AureliaUiTestRouteStatus) {
    patch_text(root, "status.route.value", if route_status.route_available { "engine.ui active" } else { "waiting" });
    patch_text(root, "status.capability.value", if route_status.ui_backend_capability { "ui.backend yes" } else { "missing" });
    patch_text(root, "status.transport.value", route_status.frame_mode_text());
    patch_text(root, "status.frame.value", &frame_index.to_string());
    patch_text(root, "status.clicks.value", &click_count.to_string());
    patch_text(root, "diag.4", &format!("✓ frame transport: {}", route_status.frame_mode_text()));
    let pulse = ((frame_index % 180) as f32) / 179.0;
    patch_value(root, "showcase.slider", &format!("{pulse:.2}"));
    patch_value(root, "showcase.progress", &format!("{:.0}%", pulse * 100.0));
}

fn patch_text(node: &mut UiNodeRequest, id: &str, text: &str) {
    if node.id == id {
        node.text = text.to_owned();
        return;
    }
    for child in &mut node.children {
        patch_text(child, id, text);
    }
}

fn patch_value(node: &mut UiNodeRequest, id: &str, value: &str) {
    if node.id == id {
        node.value = Some(value.to_owned());
        node.props.insert("value".to_owned(), serde_json::json!(value));
        return;
    }
    for child in &mut node.children {
        patch_value(child, id, value);
    }
}
