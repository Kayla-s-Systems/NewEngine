use super::*;

pub(crate) fn assets_catalog_error_node(frame_index: u64, error: String) -> UiSurfaceNode {
    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("engine.assets data unavailable")
        .with_body_lines(vec![
            "The UI projection could not read backend asset data.".to_owned(),
            error.clone(),
            "Nothing is rendered outside engine.ui; this is a normal retained node.".to_owned(),
        ])
        .with_footer_lines(vec![
            "Backend must expose data; UI decides presentation.".to_owned()
        ])
        .with_style(assets_catalog_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_message(UiNodeMessage::new(
            "Assets data unavailable",
            error,
            UiNodeMessageSeverity::Warning,
        ))
        .with_metric("frame_index", json!(frame_index));
    node.modal = true;
    node.z_order = 220;
    node.style_tags = vec![
        "workspace".to_owned(),
        "explorer-grid".to_owned(),
        "asset-catalog".to_owned(),
        "warning".to_owned(),
    ];
    node
}
