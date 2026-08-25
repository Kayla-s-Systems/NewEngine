use super::*;

use super::super::ymap_read_diagnostics::log_loaded_profile_summary;
use newengine_authored_xml as authored_xml;

pub(super) fn parse_ymap_xml_payload(
    payload: &[u8],
    logical_path: &str,
) -> Result<serde_json::Value, String> {
    let text = std::str::from_utf8(payload)
        .map_err(|e| format!("ymap XML body is not UTF-8 path='{logical_path}' err='{e}'"))?;
    let doc = authored_xml::parse_xml_document(text, &format!("ymap path='{logical_path}'"))?;
    let root = doc.root_element();
    if !root.has_tag_name("YmapMapDefinition") && !root.has_tag_name("MapDefinition") {
        return Err(format!(
            "ymap XML root must be <YmapMapDefinition> path='{logical_path}' actual='{}'",
            root.tag_name().name()
        ));
    }
    let schema = root.attribute("schema").unwrap_or_default();
    if !schema.starts_with("newengine.map.definition.") {
        return Err(format!("ymap unsupported XML schema path='{logical_path}' schema='{schema}' expected='newengine.map.definition.*'"));
    }
    let child_elements = root.children().filter(|child| child.is_element()).count();
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: XML accepted path='{}' payload_bytes={} root='{}' schema='{}' child_elements={}",
        logical_path,
        payload.len(),
        root.tag_name().name(),
        schema,
        child_elements,
    );

    let mut root_json = serde_json::Map::new();
    root_json.insert(
        "schema".to_owned(),
        serde_json::Value::String(schema.to_owned()),
    );
    if let Some(map_node) =
        authored_xml::xml_child(root, "map").or_else(|| authored_xml::xml_child(root, "Map"))
    {
        root_json.insert("map".to_owned(), ymap_node_object(map_node));
    } else if let Some(profile_node) = authored_xml::xml_child(root, "profile")
        .or_else(|| authored_xml::xml_child(root, "Profile"))
    {
        root_json.insert("profile".to_owned(), ymap_node_object(profile_node));
    } else {
        return Err(format!(
            "ymap XML has no <map> or <profile> node path='{logical_path}'"
        ));
    }
    Ok(serde_json::Value::Object(root_json))
}

pub(super) fn parse_map_definition_payload(
    value: serde_json::Value,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    let schema = value
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !schema.is_empty() && !schema.starts_with("newengine.map.definition.") {
        return Err(format!(
            "ymap unsupported schema path='{logical_path}' schema='{schema}' expected='newengine.map.definition.*'"
        ));
    }

    if let Some(profile) = value.pointer("/map/profile").cloned() {
        return parse_payload(profile, "ymap.map.profile", logical_path);
    }
    if let Some(profile) = value.get("profile").cloned() {
        return parse_payload(profile, "ymap.profile", logical_path);
    }
    if value.get("scene").is_some() {
        return Err(format!(
            "ymap scene payload rejected path='{logical_path}' policy='use newengine.map.definition.* with map.profile / profile / payload'"
        ));
    }
    if let Some(payload) = value.get("payload").cloned() {
        return parse_payload(payload, "ymap.payload", logical_path);
    }
    parse_payload(value, "ymap.root", logical_path)
}

pub(super) fn parse_payload(
    value: serde_json::Value,
    source_label: &str,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    let raw: RawGameReadyPayload = serde_json::from_value(value)
        .map_err(|e| format!("map payload parse failed source='{source_label}': {e}"))?;
    let mut profile = raw.into_profile();
    for prefab in &mut profile.prefabs {
        prefab.authored_map_ref = logical_path.to_owned();
    }
    log_loaded_profile_summary(logical_path, source_label, &profile);
    Ok(profile)
}

fn ymap_node_object(node: authored_xml::XmlNode<'_, '_>) -> serde_json::Value {
    let tag = node.tag_name().name();
    if tag.eq_ignore_ascii_case("definition_refs") {
        let refs = node
            .children()
            .filter(|child| child.is_element())
            .filter_map(|child| {
                child
                    .attribute("value")
                    .or_else(|| child.attribute("ref"))
                    .map(str::trim)
            })
            .filter(|value| !value.is_empty())
            .map(|value| serde_json::Value::String(value.to_owned()))
            .collect::<Vec<_>>();
        return serde_json::Value::Array(refs);
    }
    if tag.eq_ignore_ascii_case("definitions")
        || tag.eq_ignore_ascii_case("placements")
        || tag.eq_ignore_ascii_case("prefabs")
        || tag.eq_ignore_ascii_case("policy")
        || tag.eq_ignore_ascii_case("layers")
        || tag.eq_ignore_ascii_case("surface_layers")
        || tag.eq_ignore_ascii_case("pickups")
        || tag.eq_ignore_ascii_case("targets")
        || tag.eq_ignore_ascii_case("hazards")
        || tag.eq_ignore_ascii_case("goals")
    {
        let items = node
            .children()
            .filter(|child| child.is_element())
            .map(ymap_node_object)
            .collect::<Vec<_>>();
        return serde_json::Value::Array(items);
    }
    if tag.eq_ignore_ascii_case("DefinitionRef")
        || tag.eq_ignore_ascii_case("Policy")
        || tag.eq_ignore_ascii_case("Item")
    {
        if let Some(value) = node.attribute("value").or_else(|| node.attribute("ref")) {
            return authored_xml::xml_scalar(value);
        }
    }

    let element_children = node.children().filter(|child| child.is_element()).count();
    if element_children == 0 {
        let attr_count = node.attributes().count();
        if attr_count == 1 {
            if let Some(value) = node.attribute("value") {
                return authored_xml::xml_scalar(value);
            }
        }
    }

    let mut map = serde_json::Map::new();
    for attr in node.attributes() {
        map.insert(
            attr.name().to_owned(),
            authored_xml::xml_scalar(attr.value()),
        );
    }
    for child in node.children().filter(|child| child.is_element()) {
        let key = child.tag_name().name();
        let value = ymap_node_object(child);
        ymap_insert_child(&mut map, key, value);
    }
    if map.is_empty() {
        if let Some(text) = node.text().map(str::trim).filter(|text| !text.is_empty()) {
            return authored_xml::xml_scalar(text);
        }
    }
    serde_json::Value::Object(map)
}

fn ymap_insert_child(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) {
    let key = match key {
        "Definition" => "definitions",
        "Placement" => "placements",
        "Prefab" => "prefabs",
        "Layer" | "SurfaceLayer" => "layers",
        "Pickup" => "pickups",
        "Target" => "targets",
        "Hazard" => "hazards",
        "Goal" => "goals",
        other => other,
    };
    match map.get_mut(key) {
        Some(serde_json::Value::Array(items)) => items.push(value),
        Some(existing) => {
            let old = std::mem::replace(existing, serde_json::Value::Null);
            *existing = serde_json::Value::Array(vec![old, value]);
        }
        None => {
            map.insert(key.to_owned(), value);
        }
    }
}
