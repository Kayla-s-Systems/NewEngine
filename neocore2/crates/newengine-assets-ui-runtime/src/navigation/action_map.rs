use super::*;

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
