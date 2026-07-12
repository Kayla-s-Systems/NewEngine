use super::node::parse_ui_node_element;
use super::*;

pub(crate) fn compile_surface_root(
    xml: &str,
    surface: &SurfaceInfo,
    source_ref: &str,
    style_ref: Option<&str>,
    dialect: &NeUiDialect,
) -> Result<UiNodeRequest, String> {
    let layout = layout_by_name(xml, &surface.root)
        .or_else(|| first_element(xml, "Layout"))
        .ok_or_else(|| {
            let span = source_span_for_named_element(xml, "Surface", source_ref);
            format!(
                "{} entry='@{}' {}: .neui surface '{}' points to missing layout '{}'",
                source_ref,
                surface.root,
                span.display(source_ref),
                surface.name,
                surface.root
            )
        })?;

    let mut root = UiNodeRequest::new(surface.name.clone(), UiRuntimeNodeKind::Surface);
    root.component_id = UI_COMPONENT_SURFACE.to_owned();
    root.role = attr_value(&layout.open, "role").unwrap_or_else(|| "surface".to_owned());
    root.text = attr_value(&layout.open, "title").unwrap_or_else(|| surface.name.clone());
    root.visible = !bool_attr(&layout.open, "hidden");
    root.interactive = false;
    root.source_span = Some(source_span_for_open(xml, &layout.open, source_ref));
    root.style_tags.extend([
        "surface-root".to_owned(),
        format!("surface:{}", sanitize_tag(&surface.name)),
    ]);
    if !surface.kind.trim().is_empty() {
        root.style_tags
            .push(format!("surface-kind:{}", sanitize_tag(&surface.kind)));
    }
    root.props.insert(
        "surface_id".to_owned(),
        serde_json::Value::String(surface.name.clone()),
    );
    root.props.insert(
        "surface_kind".to_owned(),
        serde_json::Value::String(surface.kind.clone()),
    );
    root.props.insert(
        "root_layout".to_owned(),
        serde_json::Value::String(surface.root.clone()),
    );
    root.props
        .insert("modal".to_owned(), serde_json::Value::Bool(surface.modal));
    root.props
        .insert("z_order".to_owned(), serde_json::json!(surface.z_order));
    for (key, value) in parse_attrs(&layout.open) {
        if matches!(
            key.as_str(),
            "name" | "surface" | "role" | "title" | "hidden"
        ) {
            continue;
        }
        root.props
            .insert(key, serde_json::Value::String(xml_unescape(&value)));
    }
    if let Some(theme) = surface.theme.as_ref().filter(|it| !it.trim().is_empty()) {
        root.props.insert(
            "theme_ref".to_owned(),
            serde_json::Value::String(theme.clone()),
        );
    }
    if let Some(style_ref) = style_ref.filter(|it| !it.trim().is_empty()) {
        root.props.insert(
            "style_ref".to_owned(),
            serde_json::Value::String(style_ref.to_owned()),
        );
    }
    root.children = parse_layout_children(xml, &layout, source_ref, 0, dialect);
    if root.children.is_empty() {
        let span = source_span_for_open(xml, &layout.open, source_ref);
        return Err(format!(
            "{} entry='@{}' {}: .neui surface '{}' layout '{}' compiled to an empty root",
            source_ref,
            surface.root,
            span.display(source_ref),
            surface.name,
            surface.root
        ));
    }
    Ok(root)
}

pub(crate) fn parse_layout_children(
    xml: &str,
    layout: &XmlElement,
    source_ref: &str,
    depth: usize,
    dialect: &NeUiDialect,
) -> Vec<UiNodeRequest> {
    direct_child_elements(&layout.inner)
        .into_iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            parse_ui_node_element(xml, &child, source_ref, idx, depth + 1, dialect)
        })
        .collect()
}

pub(crate) fn layout_by_name(xml: &str, name: &str) -> Option<XmlElement> {
    elements(xml, "Layout")
        .into_iter()
        .find(|layout| attr_value(&layout.open, "name").as_deref() == Some(name))
}
