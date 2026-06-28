use super::*;

pub(crate) fn derive_navigation_document_from_surface_layout(
    xml: &str,
    surface: &SurfaceInfo,
) -> Result<Option<UiNodeNavigationDocument>, String> {
    let Some(layout) = elements(xml, "Layout")
        .into_iter()
        .find(|layout| attr_value(&layout.open, "name").as_deref() == Some(surface.root.as_str()))
        .or_else(|| first_element(xml, "Layout"))
    else {
        return Ok(None);
    };

    let buttons = elements(&layout.inner, "Button");
    if buttons.is_empty() {
        return Ok(None);
    }

    let routes = action_map_routes(xml);
    let mut items = Vec::new();
    for (idx, button) in buttons.into_iter().enumerate() {
        let id = attr_value(&button.open, "id").unwrap_or_else(|| format!("ui.item.{idx}"));
        let label = attr_value(&button.open, "label")
            .or_else(|| {
                first_element(&button.inner, "Text")
                    .and_then(|text| attr_value(&text.open, "value"))
            })
            .unwrap_or_else(|| id.clone());
        if label.trim().is_empty() {
            continue;
        }

        let action_id = first_element(&button.inner, "Event")
            .and_then(|event| attr_value(&event.open, "action"));
        let action = action_id
            .as_deref()
            .and_then(|id| routes.get(id).cloned())
            .or_else(|| action_id.as_deref().map(default_route_for_action_id));
        let class = attr_value(&button.open, "class")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let tone = if class.contains("primary") || idx == 0 {
            UiNodeNavigationTone::Accent
        } else {
            UiNodeNavigationTone::Normal
        };

        items.push(UiNodeNavigationItem {
            id,
            label,
            value: None,
            detail: None,
            emphasized: class.contains("primary"),
            tone,
            dynamic_value: None,
            action,
            nav_left: None,
            nav_right: None,
        });
    }

    if items.is_empty() {
        return Ok(None);
    }

    let title = first_text_with_class(&layout.inner, "title")
        .or_else(|| attr_value(&layout.open, "title"))
        .unwrap_or_else(|| surface.name.clone());

    let doc = UiNodeNavigationDocument {
        id: "engine.ui.primary".to_owned(),
        version: 1,
        surface_id: surface.name.clone(),
        root_page: "root".to_owned(),
        title,
        subtitle: "Declarative .neui layout projected as a navigation document".to_owned(),
        footer_lines: vec![
            "ESC / START - Close menu".to_owned(),
            "ARROWS / DPAD - Navigate".to_owned(),
            "ENTER / A - Confirm".to_owned(),
        ],
        pages: vec![UiNodeNavigationPage {
            id: "root".to_owned(),
            title: "Main Menu".to_owned(),
            subtitle: String::new(),
            parent_page: None,
            footer_lines: Vec::new(),
            items,
            back_route: Some(UiNodeActionRoute {
                id: "ui.close".to_owned(),
                source: "engine.assets.ui".to_owned(),
                target: "UiNodeNavigationRuntime".to_owned(),
                event: "ui.close".to_owned(),
                payload: BTreeMap::new(),
                transition: Some(UiNodeTransition::close()),
                feedback: None,
                audio: Some("ui.close".to_owned()),
            }),
        }],
    }
    .canonicalized();
    doc.validate()?;
    Ok(Some(doc))
}

pub(crate) fn action_map_routes(xml: &str) -> BTreeMap<String, UiNodeActionRoute> {
    let mut out = BTreeMap::new();
    for action in elements(xml, "Action") {
        let Some(id) = attr_value(&action.open, "id") else {
            continue;
        };
        out.insert(id.clone(), route_from_action_map_element(&id, &action));
    }
    out
}

pub(crate) fn route_from_action_map_element(id: &str, element: &XmlElement) -> UiNodeActionRoute {
    let target =
        attr_value(&element.open, "target").unwrap_or_else(|| "UiNodeNavigationRuntime".to_owned());
    let command = attr_value(&element.open, "command")
        .or_else(|| attr_value(&element.open, "event"))
        .unwrap_or_else(|| "ui.activate".to_owned());
    let mut payload = BTreeMap::new();
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(value));
        }
    }
    let transition = match command.as_str() {
        "ui.close" | "menu.close" | "engine.ui.close" => Some(UiNodeTransition::close()),
        "menu.open_page" | "ui.open_page" => payload
            .get("page")
            .and_then(serde_json::Value::as_str)
            .map(UiNodeTransition::open_page),
        _ => None,
    };
    UiNodeActionRoute {
        id: id.to_owned(),
        source: "engine.assets.ui".to_owned(),
        target,
        event: command,
        payload,
        transition,
        feedback: None,
        audio: Some("ui.select".to_owned()),
    }
}

pub(crate) fn default_route_for_action_id(id: &str) -> UiNodeActionRoute {
    UiNodeActionRoute {
        id: id.to_owned(),
        source: "engine.assets.ui".to_owned(),
        target: "UiNodeNavigationRuntime".to_owned(),
        event: id.to_owned(),
        payload: BTreeMap::new(),
        transition: None,
        feedback: None,
        audio: Some("ui.select".to_owned()),
    }
}

pub(crate) fn first_text_with_class(xml: &str, class_name: &str) -> Option<String> {
    elements(xml, "Text")
        .into_iter()
        .find(|text| {
            attr_value(&text.open, "class")
                .map(|class| class.split_whitespace().any(|token| token == class_name))
                .unwrap_or(false)
        })
        .and_then(|text| attr_value(&text.open, "value"))
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn parse_navigation_document(
    xml: &str,
) -> Result<Option<UiNodeNavigationDocument>, String> {
    let Some(navigation) = first_element(xml, "UiNodeNavigationDocument") else {
        return Ok(None);
    };
    let mut doc = UiNodeNavigationDocument {
        id: attr_value(&navigation.open, "id").unwrap_or_else(|| "engine.ui.primary".to_owned()),
        version: attr_value(&navigation.open, "version")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        surface_id: attr_value(&navigation.open, "surface_id")
            .or_else(|| attr_value(&navigation.open, "surface"))
            .unwrap_or_else(|| "engine.ui.primary".to_owned()),
        root_page: attr_value(&navigation.open, "root_page").unwrap_or_else(|| "root".to_owned()),
        title: attr_value(&navigation.open, "title").unwrap_or_default(),
        subtitle: attr_value(&navigation.open, "subtitle").unwrap_or_default(),
        footer_lines: Vec::new(),
        pages: Vec::new(),
    };

    if let Some(footer) = first_element(&navigation.inner, "Footer") {
        for line in elements(&footer.inner, "Line") {
            if let Some(value) = attr_value(&line.open, "value") {
                if !value.trim().is_empty() {
                    doc.footer_lines.push(value);
                }
            }
        }
    }

    for page_element in elements(&navigation.inner, "Page") {
        let mut page = UiNodeNavigationPage {
            id: attr_value(&page_element.open, "id").unwrap_or_default(),
            title: attr_value(&page_element.open, "title").unwrap_or_default(),
            subtitle: attr_value(&page_element.open, "subtitle").unwrap_or_default(),
            parent_page: attr_value(&page_element.open, "parent_page"),
            footer_lines: Vec::new(),
            items: Vec::new(),
            back_route: first_route_element(&page_element.inner, "Back"),
        };
        if let Some(footer) = first_element(&page_element.inner, "Footer") {
            for line in elements(&footer.inner, "Line") {
                if let Some(value) = attr_value(&line.open, "value") {
                    page.footer_lines.push(value);
                }
            }
        }
        for item_element in elements(&page_element.inner, "Item") {
            let item = UiNodeNavigationItem {
                id: attr_value(&item_element.open, "id").unwrap_or_default(),
                label: attr_value(&item_element.open, "label").unwrap_or_default(),
                value: attr_value(&item_element.open, "value"),
                detail: attr_value(&item_element.open, "detail"),
                emphasized: bool_attr(&item_element.open, "emphasized"),
                tone: tone_from_attr(attr_value(&item_element.open, "tone").as_deref()),
                dynamic_value: attr_value(&item_element.open, "dynamic_value"),
                action: first_route_element(&item_element.inner, "Action"),
                nav_left: first_route_element(&item_element.inner, "NavLeft"),
                nav_right: first_route_element(&item_element.inner, "NavRight"),
            };
            page.items.push(item);
        }
        doc.pages.push(page);
    }
    doc = doc.canonicalized();
    doc.validate()?;
    Ok(Some(doc))
}

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
