use super::layout::apply_layout_attrs;
use super::span::*;
use super::surface::{layout_by_name, parse_layout_children};
use super::*;

pub(crate) fn parse_ui_node_element(
    xml: &str,
    element: &XmlElement,
    source_ref: &str,
    generated_index: usize,
    depth: usize,
    dialect: &NeUiDialect,
) -> Option<UiNodeRequest> {
    if dialect.is_metadata_element(&element.name) || depth > 48 {
        return None;
    }

    let (kind, mut implicit_tags) = dialect.kind_for_tag(&element.name);
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
        bool_attr(&element.open, "interactive") || dialect.is_intrinsically_interactive(kind);
    node.tone = tone_from_node_attrs(&element.open);

    node.style_tags.extend(class_tags(&element.open));
    node.style_tags.push(sanitize_tag(&element.name));
    node.style_tags.append(&mut implicit_tags);
    node.style_tags.sort();
    node.style_tags.dedup();

    for (key, value) in parse_attrs(&element.open) {
        if dialect.is_structural_attr(&key) {
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
                    parse_ui_node_element(xml, &child, source_ref, idx, depth + 1, dialect)
                {
                    node.children.push(child_node);
                }
            }
        }
    }

    if let Some(use_layout) = attr_value(&element.open, "use").filter(|it| !it.trim().is_empty()) {
        if let Some(layout) = layout_by_name(xml, &use_layout) {
            node.children.extend(parse_layout_children(
                xml,
                &layout,
                source_ref,
                depth + 1,
                dialect,
            ));
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
