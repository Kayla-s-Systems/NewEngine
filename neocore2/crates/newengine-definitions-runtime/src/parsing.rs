use super::*;

fn xml_attr_string(node: authored_xml::XmlNode<'_, '_>, names: &[&str]) -> Option<String> {
    authored_xml::xml_attr_any(node, names)
        .map(|value| value.trim().replace('\\', "/"))
        .filter(|value| !value.is_empty())
}

fn xml_attr_bool(node: authored_xml::XmlNode<'_, '_>, names: &[&str], default: bool) -> bool {
    xml_attr_string(node, names)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "required"
            )
        })
        .unwrap_or(default)
}

fn xml_attr_u64(node: authored_xml::XmlNode<'_, '_>, names: &[&str]) -> u64 {
    xml_attr_string(node, names)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default()
}

fn xml_attr_u32(node: authored_xml::XmlNode<'_, '_>, names: &[&str]) -> u32 {
    xml_attr_string(node, names)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_default()
}

fn xml_tags(container: Option<authored_xml::XmlNode<'_, '_>>) -> Vec<String> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Tag"))
        .filter_map(|tag| xml_attr_string(tag, &["value", "name", "tag"]))
        .collect()
}

fn xml_dependencies(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> Vec<AssetDependencyRecord> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Dependency"))
        .filter_map(|dep| {
            let reference = xml_attr_string(dep, &["reference", "ref", "path"])?;
            let role =
                xml_attr_string(dep, &["role", "kind"]).unwrap_or_else(|| "dependency".to_owned());
            let domain = xml_attr_string(dep, &["domain"]).unwrap_or_default();
            Some(AssetDependencyRecord::new(
                reference,
                role,
                domain,
                xml_attr_bool(dep, &["required"], true),
            ))
        })
        .collect()
}

fn xml_material_bindings(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> Vec<MaterialBindingRef> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Binding"))
        .filter_map(|binding| {
            let slot = xml_attr_string(binding, &["slot", "name"])?;
            let material_ref = xml_attr_string(binding, &["material_ref", "material", "ref"])?;
            Some(MaterialBindingRef {
                slot,
                material_ref,
                required: xml_attr_bool(binding, &["required"], true),
            })
        })
        .collect()
}

fn xml_side_effects(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> Vec<DefinitionSideEffectV1> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("SideEffect"))
        .map(|effect| DefinitionSideEffectV1 {
            domain: xml_attr_string(effect, &["domain"]).unwrap_or_default(),
            effect: xml_attr_string(effect, &["effect", "name"]).unwrap_or_default(),
            target: xml_attr_string(effect, &["target"]).unwrap_or_default(),
            metadata: BTreeMap::new(),
        })
        .collect()
}

fn xml_render_namespace_value(ns: authored_xml::XmlNode<'_, '_>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for child in ns.children().filter(|child| child.is_element()) {
        if child.has_tag_name("Value") {
            if let (Some(key), Some(value)) = (
                xml_attr_string(child, &["key", "name"]),
                xml_attr_string(child, &["value"]),
            ) {
                map.insert(key, serde_json::Value::String(value));
            }
        } else {
            map.insert(
                child.tag_name().name().to_owned(),
                authored_xml::xml_node_object(child),
            );
        }
    }
    serde_json::Value::Object(map)
}

fn xml_metadata_namespaces(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    let Some(container) = container else {
        return out;
    };
    for ns in container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Namespace"))
    {
        let Some(name) = xml_attr_string(ns, &["name", "namespace"]) else {
            continue;
        };
        let value = if name == "render" || name == "newengine.render" {
            xml_render_namespace_value(ns)
        } else {
            authored_xml::xml_node_children_object(ns)
        };
        out.insert(name, value);
    }
    out
}

pub(super) fn parse_ytyp_xml_document(
    source: &str,
    body: &[u8],
) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let document = authored_xml::parse_xml_body(body, source)?;
    let root = document.root_element();
    if !authored_xml::root_has_any_name(
        root,
        &["YtypProperties", "YtypDictionary", "DefinitionEntry"],
    ) {
        return Err(format!(
            "engine.assets.definitions: unsupported .ytyp XML root source='{source}' actual='{}'",
            root.tag_name().name()
        ));
    }
    let mut raw = RawDefinitionEntryV1 {
        name: xml_attr_string(root, &["name", "id", "asset_name"]).unwrap_or_else(|| {
            source
                .rsplit('/')
                .next()
                .unwrap_or(source)
                .trim_end_matches(".ytyp")
                .to_owned()
        }),
        stable_hash: xml_attr_u64(root, &["stable_hash", "stableHash"]),
        entry_kind: xml_attr_string(root, &["entry_kind", "entryKind"])
            .unwrap_or_else(|| "archetype_definition".to_owned()),
        kind: xml_attr_string(root, &["kind"]).unwrap_or_default(),
        schema: xml_attr_string(root, &["schema"])
            .unwrap_or_else(|| "newengine.ytyp.properties.v1".to_owned()),
        flags: xml_attr_u32(root, &["flags"]),
        ..Default::default()
    };
    raw.dependencies = xml_dependencies(authored_xml::xml_child(root, "Dependencies"));
    raw.material_bindings =
        xml_material_bindings(authored_xml::xml_child(root, "MaterialBindings"));
    raw.semantic_tags = xml_tags(authored_xml::xml_child(root, "SemanticTags"));
    raw.domain_tags = xml_tags(authored_xml::xml_child(root, "DomainTags"));
    raw.namespaces = authored_xml::xml_child(root, "Namespaces")
        .map(authored_xml::xml_namespace_map)
        .unwrap_or_default();
    raw.metadata = xml_metadata_namespaces(authored_xml::xml_child(root, "Metadata"));
    raw.side_effects = xml_side_effects(authored_xml::xml_child(root, "SideEffects"));
    Ok((
        vec![raw],
        vec![format!(
            ".ytyp parsed as XML authoring schema='{}' source='{}'",
            authored_xml::root_schema(root),
            source
        )],
    ))
}

fn json_string_at(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn raw_definition_entry_from_json(
    source: &str,
    value: &serde_json::Value,
) -> Result<RawDefinitionEntryV1, String> {
    let mut raw = serde_json::from_value::<RawDefinitionEntryV1>(value.clone()).map_err(|error| {
        format!("engine.assets.definitions: invalid .ytyp JSON entry source='{source}' err='{error}'")
    })?;
    if raw.name.trim().is_empty() {
        raw.name = json_string_at(value, &["identity", "name"])
            .or_else(|| json_string_at(value, &["asset", "name"]))
            .or_else(|| json_string_at(value, &["archetype", "name"]))
            .unwrap_or_default();
    }
    if raw.schema.trim().is_empty() {
        raw.schema = json_string_at(value, &["schema"])
            .unwrap_or_else(|| "newengine.ytyp.definition_entry.v1".to_owned());
    }
    Ok(raw)
}

fn parse_ytyp_json_entries(
    source: &str,
    value: &serde_json::Value,
) -> Result<Vec<RawDefinitionEntryV1>, String> {
    if let Some(entries) = value
        .get("entries")
        .or_else(|| value.get("definition_entries"))
        .or_else(|| value.get("definitionEntries"))
        .and_then(|v| v.as_array())
    {
        return entries
            .iter()
            .map(|entry| raw_definition_entry_from_json(source, entry))
            .collect();
    }
    if let Some(entry) = value.get("entry").or_else(|| value.get("definition_entry")) {
        return Ok(vec![raw_definition_entry_from_json(source, entry)?]);
    }
    if let Some(entries) = value.as_array() {
        return entries
            .iter()
            .map(|entry| raw_definition_entry_from_json(source, entry))
            .collect();
    }
    if value.is_object() {
        return Ok(vec![raw_definition_entry_from_json(source, value)?]);
    }
    Err(format!(
        "engine.assets.definitions: .ytyp JSON root must be object or array source='{source}'"
    ))
}

pub(super) fn parse_ytyp_json_document(
    source: &str,
    body: &[u8],
) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).map_err(|error| {
        format!(
            "engine.assets.definitions: .ytyp JSON body is invalid source='{source}' err='{error}'"
        )
    })?;
    let schema = value
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or("newengine.ytyp.dictionary.v1");
    match schema {
        "newengine.ytyp.dictionary.v1"
        | "newengine.ytyp.archetype_dictionary.v1"
        | "newengine.ytyp.definition_entry.v1"
        | "newengine.ytyp.properties.v1" => {}
        other => {
            return Err(format!(
                "engine.assets.definitions: unsupported .ytyp JSON schema source='{source}' expected='newengine.ytyp.dictionary.v1' actual='{other}'"
            ));
        }
    }
    let entries = parse_ytyp_json_entries(source, &value)?;
    if entries.is_empty() {
        return Err(format!("source='{source}' contains no .ytyp entries"));
    }
    Ok((
        entries,
        vec![format!(
            ".ytyp parsed as JSON schema='{schema}' entries_source='{}'",
            source
        )],
    ))
}
