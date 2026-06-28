use super::*;

pub(crate) fn compile_surface_root(
    xml: &str,
    surface: &SurfaceInfo,
    source_ref: &str,
    style_ref: Option<&str>,
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
    root.children = parse_layout_children(xml, &layout, source_ref, 0);
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
) -> Vec<UiNodeRequest> {
    direct_child_elements(&layout.inner)
        .into_iter()
        .enumerate()
        .filter_map(|(idx, child)| parse_ui_node_element(xml, &child, source_ref, idx, depth + 1))
        .collect()
}

pub(crate) fn parse_ui_node_element(
    xml: &str,
    element: &XmlElement,
    source_ref: &str,
    generated_index: usize,
    depth: usize,
) -> Option<UiNodeRequest> {
    if is_metadata_element(&element.name) || depth > 48 {
        return None;
    }

    let (kind, mut implicit_tags) = kind_for_neui_tag(&element.name);
    let id = attr_value(&element.open, "id")
        .or_else(|| attr_value(&element.open, "name"))
        .unwrap_or_else(|| format!("{}.{}", sanitize_tag(&element.name), generated_index));
    let mut node = UiNodeRequest::new(id.clone(), kind);
    node.component_id = attr_value(&element.open, "component")
        .or_else(|| attr_value(&element.open, "template"))
        .unwrap_or_else(|| kind.default_component_id().to_owned());
    node.role = attr_value(&element.open, "role").unwrap_or_else(|| sanitize_tag(&element.name));
    node.source_span = Some(source_span_for_open(xml, &element.open, source_ref));
    node.text = attr_value(&element.open, "text")
        .or_else(|| attr_value(&element.open, "label"))
        .or_else(|| attr_value(&element.open, "title"))
        .or_else(|| attr_value(&element.open, "value").filter(|_| kind == UiRuntimeNodeKind::Text))
        .unwrap_or_default();
    node.value = attr_value(&element.open, "value").filter(|_| kind != UiRuntimeNodeKind::Text);
    node.detail =
        attr_value(&element.open, "detail").or_else(|| attr_value(&element.open, "subtitle"));
    node.icon = attr_value(&element.open, "icon").or_else(|| attr_value(&element.open, "texture"));
    node.font_token =
        attr_value(&element.open, "font").or_else(|| attr_value(&element.open, "font_token"));
    node.tooltip = attr_value(&element.open, "tooltip");
    node.visible = !bool_attr(&element.open, "hidden")
        && !matches!(
            attr_value(&element.open, "visible").as_deref(),
            Some("false") | Some("0") | Some("no")
        );
    node.enabled = !matches!(
        attr_value(&element.open, "enabled").as_deref(),
        Some("false") | Some("0") | Some("no")
    );
    node.interactive =
        bool_attr(&element.open, "interactive") || is_intrinsically_interactive(kind);
    node.tone = tone_from_node_attrs(&element.open);

    node.style_tags.extend(class_tags(&element.open));
    node.style_tags.push(sanitize_tag(&element.name));
    node.style_tags.append(&mut implicit_tags);
    node.style_tags.sort();
    node.style_tags.dedup();

    for (key, value) in parse_attrs(&element.open) {
        if is_structural_attr(&key) {
            continue;
        }
        node.props
            .insert(key, serde_json::Value::String(xml_unescape(&value)));
    }
    apply_layout_attrs(&mut node, &element.open);

    for (idx, child) in direct_child_elements(&element.inner)
        .into_iter()
        .enumerate()
    {
        match child.name.as_str() {
            "Bind" => {
                let binding = binding_from_element(&child);
                if node.text.trim().is_empty() && binding.property == "text" {
                    if let Some(value) =
                        binding.fallback.as_str().filter(|it| !it.trim().is_empty())
                    {
                        node.text = value.to_owned();
                    }
                }
                if node.value.is_none() && binding.property == "value" {
                    if let Some(value) =
                        binding.fallback.as_str().filter(|it| !it.trim().is_empty())
                    {
                        node.value = Some(value.to_owned());
                    }
                }
                node.bindings.push(binding);
            }
            "Event" => {
                let route = event_route_from_element(&child);
                if node.action_id.is_none()
                    && route.trigger == UiNodeEventTrigger::Click
                    && !route.action_id.trim().is_empty()
                {
                    node.action_id = Some(route.action_id.clone());
                    node.interactive = true;
                }
                node.events.push(route);
            }
            "Text"
                if matches!(kind, UiRuntimeNodeKind::Button | UiRuntimeNodeKind::Action)
                    && attr_value(&child.open, "id").is_none() =>
            {
                if node.text.trim().is_empty() {
                    node.text = attr_value(&child.open, "value")
                        .or_else(|| attr_value(&child.open, "text"))
                        .or_else(|| attr_value(&child.open, "label"))
                        .unwrap_or_default();
                }
            }
            _ => {
                if let Some(child_node) =
                    parse_ui_node_element(xml, &child, source_ref, idx, depth + 1)
                {
                    node.children.push(child_node);
                }
            }
        }
    }

    if let Some(use_layout) = attr_value(&element.open, "use").filter(|it| !it.trim().is_empty()) {
        if let Some(layout) = layout_by_name(xml, &use_layout) {
            node.children
                .extend(parse_layout_children(xml, &layout, source_ref, depth + 1));
            node.props
                .insert("use".to_owned(), serde_json::Value::String(use_layout));
        }
    }

    if node.action_id.is_none() {
        node.action_id = attr_value(&element.open, "action")
            .or_else(|| attr_value(&element.open, "action_id"))
            .or_else(|| attr_value(&element.open, "command"));
        if node.action_id.is_some() {
            node.interactive = true;
        }
    }
    if matches!(kind, UiRuntimeNodeKind::Action) && node.action_id.is_none() {
        let value = node.value.clone().unwrap_or_else(|| id.clone());
        node.action_id = Some(format!("ui.select.{value}"));
        node.interactive = true;
    }
    if node.text.trim().is_empty()
        && matches!(kind, UiRuntimeNodeKind::Action | UiRuntimeNodeKind::Button)
    {
        node.text = id.clone();
    }

    Some(node)
}

pub(crate) fn layout_by_name(xml: &str, name: &str) -> Option<XmlElement> {
    elements(xml, "Layout")
        .into_iter()
        .find(|layout| attr_value(&layout.open, "name").as_deref() == Some(name))
}

pub(crate) fn is_metadata_element(name: &str) -> bool {
    matches!(
        name,
        "Entries"
            | "Entry"
            | "Surface"
            | "Dependencies"
            | "ThemeRef"
            | "ComponentRef"
            | "TextureRef"
            | "FontRef"
            | "SoundRef"
            | "BindingGraph"
            | "StateSource"
            | "Bind"
            | "ActionMap"
            | "Action"
            | "Event"
            | "Payload"
            | "Slot"
            | "UiNodeNavigationDocument"
            | "Page"
            | "Footer"
            | "Line"
            | "NavLeft"
            | "NavRight"
            | "Back"
    )
}

pub(crate) fn kind_for_neui_tag(name: &str) -> (UiRuntimeNodeKind, Vec<String>) {
    let normalized = sanitize_tag(name).replace('-', "");
    match normalized.as_str() {
        "surface" => (
            UiRuntimeNodeKind::Surface,
            vec![UI_COMPONENT_SURFACE.to_owned()],
        ),
        "panel" | "card" | "statuscard" | "metriccard" | "warningcard" | "plugincard"
        | "propertycard" => (UiRuntimeNodeKind::Panel, vec![sanitize_tag(name)]),
        "stack" => (
            UiRuntimeNodeKind::Stack,
            vec![UI_COMPONENT_STACK.to_owned()],
        ),
        "row" => (UiRuntimeNodeKind::Row, vec![UI_COMPONENT_ROW.to_owned()]),
        "column" | "col" => (UiRuntimeNodeKind::Column, vec!["column".to_owned()]),
        "grid" => (UiRuntimeNodeKind::Grid, vec![UI_COMPONENT_GRID.to_owned()]),
        "text" | "label" => (UiRuntimeNodeKind::Text, vec![UI_COMPONENT_TEXT.to_owned()]),
        "button" => (
            UiRuntimeNodeKind::Button,
            vec![UI_COMPONENT_BUTTON.to_owned()],
        ),
        "action" | "option" | "item" | "selectitem" | "dropdownitem" | "menuitem" => (
            UiRuntimeNodeKind::Action,
            vec![
                UI_COMPONENT_ACTION.to_owned(),
                "select-option".to_owned(),
                "option".to_owned(),
            ],
        ),
        "input" | "textinput" | "field" | "search" => (
            UiRuntimeNodeKind::Input,
            vec![UI_COMPONENT_INPUT.to_owned()],
        ),
        "checkbox" | "check" => (
            UiRuntimeNodeKind::Checkbox,
            vec![UI_COMPONENT_CHECKBOX.to_owned()],
        ),
        "toggle" | "switch" => (
            UiRuntimeNodeKind::Toggle,
            vec![UI_COMPONENT_TOGGLE.to_owned()],
        ),
        "slider" | "progress" | "progressbar" => (
            UiRuntimeNodeKind::Slider,
            vec![UI_COMPONENT_SLIDER.to_owned(), normalized],
        ),
        "scrollbar" => (
            UiRuntimeNodeKind::ScrollBar,
            vec![UI_COMPONENT_SCROLL_BAR.to_owned()],
        ),
        "select" | "dropdown" | "combobox" => (
            UiRuntimeNodeKind::Select,
            vec![UI_COMPONENT_SELECT.to_owned()],
        ),
        "separator" | "divider" => (
            UiRuntimeNodeKind::Separator,
            vec![UI_COMPONENT_SEPARATOR.to_owned()],
        ),
        "list" | "propertygrid" => (
            UiRuntimeNodeKind::List,
            vec![UI_COMPONENT_LIST.to_owned(), sanitize_tag(name)],
        ),
        "tree" => (UiRuntimeNodeKind::Tree, vec![UI_COMPONENT_TREE.to_owned()]),
        "split" | "splitter" => (UiRuntimeNodeKind::Split, vec!["split".to_owned()]),
        "viewport" => (
            UiRuntimeNodeKind::Viewport,
            vec![UI_COMPONENT_VIEWPORT.to_owned()],
        ),
        "image" | "texture" | "externaltexture" | "icon" => (
            UiRuntimeNodeKind::ExternalTexture,
            vec![UI_COMPONENT_EXTERNAL_TEXTURE.to_owned()],
        ),
        "spacer" => (
            UiRuntimeNodeKind::Spacer,
            vec![UI_COMPONENT_SPACER.to_owned()],
        ),
        _ => (
            UiRuntimeNodeKind::Panel,
            vec!["custom".to_owned(), sanitize_tag(name)],
        ),
    }
}

pub(crate) fn is_intrinsically_interactive(kind: UiRuntimeNodeKind) -> bool {
    matches!(
        kind,
        UiRuntimeNodeKind::Action
            | UiRuntimeNodeKind::Button
            | UiRuntimeNodeKind::Input
            | UiRuntimeNodeKind::Checkbox
            | UiRuntimeNodeKind::Toggle
            | UiRuntimeNodeKind::Slider
            | UiRuntimeNodeKind::ScrollBar
            | UiRuntimeNodeKind::Select
            | UiRuntimeNodeKind::List
            | UiRuntimeNodeKind::Tree
            | UiRuntimeNodeKind::Split
            | UiRuntimeNodeKind::Viewport
    )
}

pub(crate) fn binding_from_element(element: &XmlElement) -> UiNodeBindingRequest {
    let source = attr_value(&element.open, "source").unwrap_or_default();
    let (source_id, path) = if let Some((source_id, path)) = source.split_once('.') {
        (source_id.to_owned(), path.to_owned())
    } else {
        (String::new(), source.clone())
    };
    UiNodeBindingRequest {
        property: attr_value(&element.open, "property").unwrap_or_else(|| "value".to_owned()),
        source: source_id,
        path,
        mode: attr_value(&element.open, "mode").unwrap_or_else(|| "read".to_owned()),
        fallback: attr_value(&element.open, "fallback")
            .map(json_value_from_attr)
            .unwrap_or(serde_json::Value::Null),
    }
}

pub(crate) fn event_route_from_element(element: &XmlElement) -> UiNodeEventRoute {
    let mut payload = serde_json::Map::new();
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(xml_unescape(&value)));
        }
    }
    UiNodeEventRoute {
        trigger: trigger_from_attr(attr_value(&element.open, "trigger").as_deref()),
        action_id: attr_value(&element.open, "action")
            .or_else(|| attr_value(&element.open, "action_id"))
            .or_else(|| attr_value(&element.open, "id"))
            .unwrap_or_default(),
        target_gateway: attr_value(&element.open, "target")
            .unwrap_or_else(|| newengine_ui_api::ENGINE_UI_SERVICE_ID.to_owned()),
        method: attr_value(&element.open, "method")
            .unwrap_or_else(|| newengine_ui_api::UI_SERVICE_METHOD_DISPATCH_ACTION_V1.to_owned()),
        payload: serde_json::Value::Object(payload),
    }
}

pub(crate) fn trigger_from_attr(value: Option<&str>) -> UiNodeEventTrigger {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "hover_enter" | "mouseenter" => UiNodeEventTrigger::HoverEnter,
        "hover_exit" | "mouseleave" => UiNodeEventTrigger::HoverExit,
        "press" | "pointer_down" => UiNodeEventTrigger::Press,
        "release" | "pointer_up" => UiNodeEventTrigger::Release,
        "double_click" | "dblclick" => UiNodeEventTrigger::DoubleClick,
        "focus" => UiNodeEventTrigger::Focus,
        "blur" => UiNodeEventTrigger::Blur,
        "value_changed" | "change" => UiNodeEventTrigger::ValueChanged,
        "drag_start" => UiNodeEventTrigger::DragStart,
        "drag_move" => UiNodeEventTrigger::DragMove,
        "drag_end" => UiNodeEventTrigger::DragEnd,
        "context_menu" => UiNodeEventTrigger::ContextMenu,
        _ => UiNodeEventTrigger::Click,
    }
}

pub(crate) fn tone_from_node_attrs(open: &str) -> UiNodeTone {
    let tone = attr_value(open, "tone")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let classes = attr_value(open, "class")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if tone == "danger" || classes.contains("danger") {
        UiNodeTone::Danger
    } else if tone == "accent"
        || tone == "primary"
        || classes.contains("primary")
        || classes.contains("accent")
    {
        UiNodeTone::Accent
    } else if tone == "disabled" || classes.contains("disabled") {
        UiNodeTone::Disabled
    } else {
        UiNodeTone::Normal
    }
}

pub(crate) fn class_tags(open: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for token in attr_value(open, "class")
        .unwrap_or_default()
        .split_whitespace()
    {
        let tag = sanitize_tag(token);
        if tag.is_empty() {
            continue;
        }
        tags.push(tag.clone());
        for prefix in ["button-", "ui-", "aurelia-", "dark-", "light-"] {
            if let Some(rest) = tag.strip_prefix(prefix).filter(|it| !it.is_empty()) {
                tags.push(rest.to_owned());
            }
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

pub(crate) fn sanitize_tag(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

pub(crate) fn source_span_for_named_element(
    xml: &str,
    name: &str,
    source_ref: &str,
) -> UiSourceSpan {
    first_element(xml, name)
        .map(|element| source_span_for_open(xml, &element.open, source_ref))
        .unwrap_or_else(|| source_span_for_offset(xml, 0, source_ref))
}

pub(crate) fn source_span_for_open(xml: &str, open: &str, source_ref: &str) -> UiSourceSpan {
    let offset = xml.find(open).unwrap_or(0);
    source_span_for_offset(xml, offset, source_ref)
}

pub(crate) fn source_span_for_offset(xml: &str, offset: usize, source_ref: &str) -> UiSourceSpan {
    let mut line = 1u32;
    let mut column = 1u32;
    for ch in xml[..offset.min(xml.len())].chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
            column = 1;
        } else {
            column = column.saturating_add(1);
        }
    }
    UiSourceSpan {
        source_ref: source_ref.to_owned(),
        line,
        column,
    }
}

fn attr_f32(open: &str, keys: &[&str]) -> Option<f32> {
    keys.iter()
        .find_map(|key| attr_value(open, key))
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
}

fn attr_i32(open: &str, keys: &[&str]) -> Option<i32> {
    keys.iter()
        .find_map(|key| attr_value(open, key))
        .and_then(|value| value.trim().parse::<i32>().ok())
}

fn attr_bool_value(open: &str, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        let value = attr_value(open, key)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn put_f32_prop(node: &mut UiNodeRequest, key: &str, value: f32) {
    if let Some(number) = serde_json::Number::from_f64(value as f64) {
        node.props
            .insert(key.to_owned(), serde_json::Value::Number(number));
    }
}

fn put_bool_prop(node: &mut UiNodeRequest, key: &str, value: bool) {
    node.props
        .insert(key.to_owned(), serde_json::Value::Bool(value));
}

fn apply_layout_attrs(node: &mut UiNodeRequest, open: &str) {
    if let Some(value) = attr_f32(open, &["x_px", "x"]) {
        node.layout.x_px = Some(value);
        put_f32_prop(node, "x_px", value);
    }
    if let Some(value) = attr_f32(open, &["y_px", "y"]) {
        node.layout.y_px = Some(value);
        put_f32_prop(node, "y_px", value);
    }
    if let Some(value) = attr_f32(open, &["w_px", "width_px", "width"]) {
        node.layout.w_px = Some(value);
        put_f32_prop(node, "w_px", value);
    }
    if let Some(value) = attr_f32(open, &["h_px", "height_px", "height"]) {
        node.layout.h_px = Some(value);
        put_f32_prop(node, "h_px", value);
    }

    if let Some(value) = attr_f32(open, &["min_w_px", "min_width_px"]) {
        node.layout.min_size_px[0] = value;
        put_f32_prop(node, "min_w_px", value);
    }
    if let Some(value) = attr_f32(open, &["min_h_px", "min_height_px"]) {
        node.layout.min_size_px[1] = value;
        put_f32_prop(node, "min_h_px", value);
    }
    if let Some(value) = attr_f32(open, &["max_w_px", "max_width_px"]) {
        node.layout.max_size_px[0] = value;
        put_f32_prop(node, "max_w_px", value);
    }
    if let Some(value) = attr_f32(open, &["max_h_px", "max_height_px"]) {
        node.layout.max_size_px[1] = value;
        put_f32_prop(node, "max_h_px", value);
    }
    if let Some(value) = attr_f32(open, &["grow", "flex_grow"]) {
        node.layout.grow = value.max(0.0);
        put_f32_prop(node, "grow", node.layout.grow);
    }
    if let Some(value) = attr_f32(open, &["shrink", "flex_shrink"]) {
        node.layout.shrink = value.max(0.0);
        put_f32_prop(node, "shrink", node.layout.shrink);
    }
    if let Some(value) = attr_i32(open, &["order"]) {
        node.layout.order = value;
        node.props
            .insert("order".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = attr_value(open, "slot").filter(|it| !it.trim().is_empty()) {
        node.layout.slot = value;
        node.props.insert(
            "slot".to_owned(),
            serde_json::Value::String(node.layout.slot.clone()),
        );
    }
    if let Some(value) = attr_bool_value(open, &["resizable"]) {
        node.layout.resizable = value;
        put_bool_prop(node, "resizable", value);
    }
    if let Some(value) = attr_bool_value(open, &["detachable"]) {
        node.layout.detachable = value;
        put_bool_prop(node, "detachable", value);
    }
}

pub(crate) fn json_value_from_attr(value: String) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        serde_json::Value::Bool(true)
    } else if trimmed.eq_ignore_ascii_case("false") {
        serde_json::Value::Bool(false)
    } else if let Ok(number) = trimmed.parse::<i64>() {
        serde_json::Value::Number(number.into())
    } else if let Ok(number) = trimmed.parse::<f64>() {
        serde_json::Number::from_f64(number)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(value))
    } else {
        serde_json::Value::String(value)
    }
}

pub(crate) fn is_structural_attr(key: &str) -> bool {
    matches!(
        key,
        "id" | "name"
            | "class"
            | "role"
            | "text"
            | "label"
            | "title"
            | "detail"
            | "subtitle"
            | "value"
            | "icon"
            | "texture"
            | "font"
            | "font_token"
            | "tooltip"
            | "hidden"
            | "visible"
            | "enabled"
            | "interactive"
            | "tone"
            | "action"
            | "action_id"
            | "command"
            | "use"
            | "component"
            | "template"
    )
}
