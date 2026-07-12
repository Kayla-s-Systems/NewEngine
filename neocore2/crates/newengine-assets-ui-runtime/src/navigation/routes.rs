use super::*;

pub(crate) fn first_route_element(xml: &str, name: &str) -> Option<UiNodeActionRoute> {
    let element = first_element(xml, name)?;
    Some(route_from_element(&element))
}

pub(crate) fn route_from_element(element: &XmlElement) -> UiNodeActionRoute {
    let mut payload = BTreeMap::new();
    if let Some(page) = attr_value(&element.open, "page") {
        payload.insert("page".to_owned(), serde_json::Value::String(page));
    }
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(value));
        }
    }
    UiNodeActionRoute {
        id: attr_value(&element.open, "id").unwrap_or_default(),
        source: attr_value(&element.open, "source").unwrap_or_default(),
        target: attr_value(&element.open, "target")
            .unwrap_or_else(|| "UiNodeNavigationRuntime".to_owned()),
        event: attr_value(&element.open, "event")
            .unwrap_or_else(|| event_from_route_tag(&element.name).to_owned()),
        payload,
        transition: transition_from_attrs(&element.open),
        feedback: first_element(&element.inner, "Feedback").map(|feedback| UiNodeFeedbackEvent {
            title: attr_value(&feedback.open, "title").unwrap_or_default(),
            detail: attr_value(&feedback.open, "detail").unwrap_or_default(),
            severity: feedback_severity_from_attr(
                attr_value(&feedback.open, "severity").as_deref(),
            ),
            ttl_sec: attr_value(&feedback.open, "ttl_sec")
                .and_then(|v| v.parse().ok())
                .unwrap_or(2.25),
        }),
        audio: attr_value(&element.open, "audio"),
    }
}

pub(crate) fn event_from_route_tag(name: &str) -> &'static str {
    match name {
        "Back" => "ui.back",
        "NavLeft" => "ui.nav_left",
        "NavRight" => "ui.nav_right",
        _ => "ui.activate",
    }
}

pub(crate) fn transition_from_attrs(open: &str) -> Option<UiNodeTransition> {
    match attr_value(open, "transition")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "close" => Some(UiNodeTransition::close()),
        "open_page" => attr_value(open, "page").map(UiNodeTransition::open_page),
        "back" => Some(UiNodeTransition {
            kind: UiNodeTransitionKind::Back,
            page: None,
            reset_selection: true,
        }),
        "none" | "" => None,
        _ => None,
    }
}

pub(crate) fn tone_from_attr(value: Option<&str>) -> UiNodeNavigationTone {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "accent" => UiNodeNavigationTone::Accent,
        "danger" => UiNodeNavigationTone::Danger,
        "disabled" => UiNodeNavigationTone::Disabled,
        _ => UiNodeNavigationTone::Normal,
    }
}

pub(crate) fn feedback_severity_from_attr(value: Option<&str>) -> UiNodeFeedbackSeverity {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "success" => UiNodeFeedbackSeverity::Success,
        "warning" => UiNodeFeedbackSeverity::Warning,
        "danger" | "error" => UiNodeFeedbackSeverity::Danger,
        _ => UiNodeFeedbackSeverity::Info,
    }
}

pub(crate) fn bool_attr(open: &str, key: &str) -> bool {
    matches!(
        attr_value(open, key)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "true" | "1" | "yes"
    )
}
