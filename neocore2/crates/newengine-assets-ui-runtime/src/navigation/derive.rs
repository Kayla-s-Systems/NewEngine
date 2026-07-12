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
