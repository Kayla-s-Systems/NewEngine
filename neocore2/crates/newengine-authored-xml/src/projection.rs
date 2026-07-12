use std::collections::BTreeMap;

use crate::{
    document::{parse_xml_document, XmlNode},
    query::{root_schema, xml_attr_any},
};

pub fn xml_namespace_map(container: XmlNode<'_, '_>) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();

    for namespace in container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Namespace"))
    {
        let Some(name) = xml_attr_any(namespace, &["name", "namespace"]) else {
            continue;
        };

        out.insert(name, xml_node_children_object(namespace));
    }

    out
}

pub fn xml_node_children_object(node: XmlNode<'_, '_>) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    insert_non_identity_attributes(&mut map, node);

    for child in node.children().filter(|child| child.is_element()) {
        xml_insert_child(&mut map, child.tag_name().name(), xml_node_object(child));
    }

    serde_json::Value::Object(map)
}

pub fn xml_node_object(node: XmlNode<'_, '_>) -> serde_json::Value {
    let element_children = node.children().filter(|child| child.is_element()).count();

    if element_children == 0 {
        let non_identity_attributes = node
            .attributes()
            .filter(|attribute| !is_identity_attribute(attribute.name()))
            .collect::<Vec<_>>();

        if non_identity_attributes.len() == 1 && non_identity_attributes[0].name() == "value" {
            return xml_scalar(non_identity_attributes[0].value());
        }
    }

    let mut map = serde_json::Map::new();
    let had_non_identity_attribute = insert_non_identity_attributes(&mut map, node);

    for child in node.children().filter(|child| child.is_element()) {
        xml_insert_child(&mut map, child.tag_name().name(), xml_node_object(child));
    }

    if map.is_empty() && !had_non_identity_attribute {
        if let Some(text) = node.text().map(str::trim).filter(|text| !text.is_empty()) {
            return xml_scalar(text);
        }
    }

    serde_json::Value::Object(map)
}

pub fn xml_insert_child(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) {
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

pub fn xml_scalar(raw: &str) -> serde_json::Value {
    let value = raw.trim();

    if value.contains(',') {
        let atoms = value
            .split(',')
            .map(|item| xml_scalar_atom(item.trim()))
            .collect::<Vec<_>>();

        // Comma is common prose punctuation in authored metadata. Treat comma-separated
        // attributes as vectors only when every item is an actual scalar atom
        // (number/bool). String lists should be expressed as child XML nodes, not as
        // ambiguous comma-delimited text.
        if atoms
            .iter()
            .all(|item| !matches!(item, serde_json::Value::String(_)))
        {
            return serde_json::Value::Array(atoms);
        }
    }

    xml_scalar_atom(value)
}

pub fn xml_to_json_projection(text: &str, label: &str) -> Result<serde_json::Value, String> {
    let doc = parse_xml_document(text, label)?;
    let root = doc.root_element();
    let mut map = serde_json::Map::new();

    map.insert(
        "root".to_owned(),
        serde_json::Value::String(root.tag_name().name().to_owned()),
    );
    map.insert(
        "schema".to_owned(),
        serde_json::Value::String(root_schema(root)),
    );
    map.insert("body".to_owned(), xml_node_object(root));

    Ok(serde_json::Value::Object(map))
}

fn insert_non_identity_attributes(
    map: &mut serde_json::Map<String, serde_json::Value>,
    node: XmlNode<'_, '_>,
) -> bool {
    let mut inserted = false;

    for attribute in node.attributes() {
        if is_identity_attribute(attribute.name()) {
            continue;
        }

        inserted = true;
        map.insert(attribute.name().to_owned(), xml_scalar(attribute.value()));
    }

    inserted
}

#[inline]
fn is_identity_attribute(name: &str) -> bool {
    name == "name" || name == "namespace"
}

fn xml_scalar_atom(value: &str) -> serde_json::Value {
    match value.to_ascii_lowercase().as_str() {
        "true" => return serde_json::Value::Bool(true),
        "false" => return serde_json::Value::Bool(false),
        _ => {}
    }

    if let Ok(integer) = value.parse::<i64>() {
        return serde_json::json!(integer);
    }

    if let Ok(unsigned) = value.parse::<u64>() {
        return serde_json::json!(unsigned);
    }

    if let Ok(float) = value.parse::<f64>() {
        if let Some(number) = serde_json::Number::from_f64(float) {
            return serde_json::Value::Number(number);
        }
    }

    serde_json::Value::String(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_scalar_keeps_comma_prose_as_string() {
        assert_eq!(
            xml_scalar("Walk data-driven ytyp, ydd, nemat and ytd assets."),
            serde_json::Value::String(
                "Walk data-driven ytyp, ydd, nemat and ytd assets.".to_owned()
            )
        );
    }

    #[test]
    fn xml_scalar_parses_numeric_vectors() {
        assert_eq!(
            xml_scalar("1.0,2.5,-3.0"),
            serde_json::json!([1.0, 2.5, -3.0])
        );
    }

    #[test]
    fn repeated_children_become_stable_arrays() {
        let projection = xml_to_json_projection(
            r#"<Root schema="test.v1"><Item value="1"/><Item value="2"/></Root>"#,
            "projection_test",
        )
        .expect("valid projection");

        assert_eq!(projection["body"]["Item"], serde_json::json!([1, 2]));
    }
}
