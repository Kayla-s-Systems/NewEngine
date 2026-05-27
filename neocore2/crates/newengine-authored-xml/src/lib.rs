#![forbid(unsafe_op_in_unsafe_fn)]

//! Central authored XML helpers for NEF8/ListFile metadata bodies.
//!
//! This crate intentionally owns only presentation-level XML mechanics:
//! UTF-8/XML detection, stable tree-to-value projection, formatting and editor
//! completion hints. Domain semantics remain in `engine.assets.definitions`,
//! `engine.scene`, `engine.assets.materials`, etc.

use std::collections::BTreeMap;

pub type XmlDocument<'input> = roxmltree::Document<'input>;
pub type XmlNode<'a, 'input> = roxmltree::Node<'a, 'input>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlCompletionCatalog {
    pub schema_family: &'static str,
    pub root_snippets: &'static [XmlSnippet],
    pub child_snippets: &'static [XmlSnippet],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlSnippet {
    pub label: &'static str,
    pub insert: &'static str,
    pub detail: &'static str,
}

#[inline]
pub fn body_is_xml(body: &[u8]) -> bool {
    std::str::from_utf8(body)
        .map(|text| text.trim_start().starts_with('<'))
        .unwrap_or(false)
}

#[inline]
pub fn text_is_xml(text: &str) -> bool { text.trim_start().starts_with('<') }

pub fn parse_xml_document<'input>(text: &'input str, label: &str) -> Result<XmlDocument<'input>, String> {
    roxmltree::Document::parse(text).map_err(|error| format!("{label}: XML parse failed: {error}"))
}

pub fn parse_xml_body<'input>(body: &'input [u8], label: &str) -> Result<XmlDocument<'input>, String> {
    let text = std::str::from_utf8(body).map_err(|error| format!("{label}: XML body is not UTF-8: {error}"))?;
    parse_xml_document(text, label)
}

#[inline]
pub fn root_schema(root: XmlNode<'_, '_>) -> String { xml_attr_any(root, &["schema"]).unwrap_or_default() }

#[inline]
pub fn root_has_any_name(root: XmlNode<'_, '_>, names: &[&str]) -> bool {
    names.iter().any(|name| root.has_tag_name(*name))
}

pub fn xml_attr_any(node: XmlNode<'_, '_>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| node.attribute(*name).map(|value| value.trim().to_owned()).filter(|value| !value.is_empty()))
}

pub fn xml_attr_bool_any(node: XmlNode<'_, '_>, names: &[&str]) -> Option<bool> {
    xml_attr_any(node, names).map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

pub fn xml_attr_u32_any(node: XmlNode<'_, '_>, names: &[&str]) -> Option<u32> {
    xml_attr_any(node, names).and_then(|value| value.parse::<u32>().ok())
}

pub fn xml_attr_u64_any(node: XmlNode<'_, '_>, names: &[&str]) -> Option<u64> {
    xml_attr_any(node, names).and_then(|value| value.parse::<u64>().ok())
}

pub fn xml_attr_f32_any(node: XmlNode<'_, '_>, names: &[&str]) -> Option<f32> {
    xml_attr_any(node, names).and_then(|value| value.parse::<f32>().ok())
}

pub fn xml_child<'a, 'input>(node: XmlNode<'a, 'input>, name: &str) -> Option<XmlNode<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name().eq_ignore_ascii_case(name))
}

pub fn xml_children_named<'a, 'input>(node: XmlNode<'a, 'input>, name: &str) -> Vec<XmlNode<'a, 'input>> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name().eq_ignore_ascii_case(name))
        .collect()
}

pub fn xml_tags(node: XmlNode<'_, '_>, container: &str) -> Vec<String> {
    xml_child(node, container)
        .map(|tags| {
            tags.children()
                .filter(|child| child.is_element())
                .filter_map(|child| xml_attr_any(child, &["value", "name"]).or_else(|| child.text().map(str::trim).map(ToOwned::to_owned)))
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn xml_namespace_map(container: XmlNode<'_, '_>) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    for ns in container.children().filter(|child| child.is_element() && child.has_tag_name("Namespace")) {
        let Some(name) = xml_attr_any(ns, &["name", "namespace"]) else { continue; };
        out.insert(name, xml_node_children_object(ns));
    }
    out
}

pub fn xml_node_children_object(node: XmlNode<'_, '_>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for attr in node.attributes() {
        let name = attr.name();
        if name == "name" || name == "namespace" { continue; }
        map.insert(name.to_owned(), xml_scalar(attr.value()));
    }
    for child in node.children().filter(|child| child.is_element()) {
        xml_insert_child(&mut map, child.tag_name().name(), xml_node_object(child));
    }
    serde_json::Value::Object(map)
}

pub fn xml_node_object(node: XmlNode<'_, '_>) -> serde_json::Value {
    let element_children = node.children().filter(|child| child.is_element()).count();
    if element_children == 0 {
        let non_name_attrs = node
            .attributes()
            .filter(|attr| attr.name() != "name" && attr.name() != "namespace")
            .collect::<Vec<_>>();
        if non_name_attrs.len() == 1 && non_name_attrs[0].name() == "value" {
            return xml_scalar(non_name_attrs[0].value());
        }
    }

    let mut map = serde_json::Map::new();
    let mut had_non_name_attr = false;
    for attr in node.attributes() {
        let name = attr.name();
        if name == "name" || name == "namespace" { continue; }
        had_non_name_attr = true;
        map.insert(name.to_owned(), xml_scalar(attr.value()));
    }
    for child in node.children().filter(|child| child.is_element()) {
        xml_insert_child(&mut map, child.tag_name().name(), xml_node_object(child));
    }
    if map.is_empty() && !had_non_name_attr {
        if let Some(text) = node.text().map(str::trim).filter(|text| !text.is_empty()) {
            return xml_scalar(text);
        }
    }
    serde_json::Value::Object(map)
}

pub fn xml_insert_child(map: &mut serde_json::Map<String, serde_json::Value>, key: &str, value: serde_json::Value) {
    match map.get_mut(key) {
        Some(serde_json::Value::Array(items)) => items.push(value),
        Some(existing) => {
            let old = std::mem::replace(existing, serde_json::Value::Null);
            *existing = serde_json::Value::Array(vec![old, value]);
        }
        None => { map.insert(key.to_owned(), value); }
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
        if atoms.iter().all(|item| !matches!(item, serde_json::Value::String(_))) {
            return serde_json::Value::Array(atoms);
        }
    }
    xml_scalar_atom(value)
}

fn xml_scalar_atom(value: &str) -> serde_json::Value {
    match value.to_ascii_lowercase().as_str() {
        "true" => return serde_json::Value::Bool(true),
        "false" => return serde_json::Value::Bool(false),
        _ => {}
    }
    if let Ok(integer) = value.parse::<i64>() { return serde_json::json!(integer); }
    if let Ok(unsigned) = value.parse::<u64>() { return serde_json::json!(unsigned); }
    if let Ok(float) = value.parse::<f64>() {
        if let Some(number) = serde_json::Number::from_f64(float) {
            return serde_json::Value::Number(number);
        }
    }
    serde_json::Value::String(value.to_owned())
}

pub fn xml_to_json_projection(text: &str, label: &str) -> Result<serde_json::Value, String> {
    let doc = parse_xml_document(text, label)?;
    let root = doc.root_element();
    let mut map = serde_json::Map::new();
    map.insert("root".to_owned(), serde_json::Value::String(root.tag_name().name().to_owned()));
    map.insert("schema".to_owned(), serde_json::Value::String(root_schema(root)));
    map.insert("body".to_owned(), xml_node_object(root));
    Ok(serde_json::Value::Object(map))
}

pub fn format_xml_lossy(text: &str) -> Result<String, String> {
    let doc = parse_xml_document(text, "format_xml")?;
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    format_node(&mut out, doc.root_element(), 0);
    Ok(out)
}

fn format_node(out: &mut String, node: XmlNode<'_, '_>, depth: usize) {
    let indent = "  ".repeat(depth);
    out.push_str(&indent);
    out.push('<');
    out.push_str(node.tag_name().name());
    for attr in node.attributes() {
        out.push(' ');
        out.push_str(attr.name());
        out.push_str("=\"");
        push_escaped(out, attr.value());
        out.push('"');
    }
    let children = node.children().filter(|child| child.is_element()).collect::<Vec<_>>();
    let text = node.text().map(str::trim).filter(|text| !text.is_empty());
    if children.is_empty() && text.is_none() {
        out.push_str(" />\n");
        return;
    }
    out.push('>');
    if let Some(text) = text {
        push_escaped(out, text);
    }
    if !children.is_empty() {
        out.push('\n');
        for child in children {
            format_node(out, child, depth + 1);
        }
        out.push_str(&indent);
    }
    out.push_str("</");
    out.push_str(node.tag_name().name());
    out.push_str(">\n");
}

fn push_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
}

static DEFINITION_ROOT_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Ytyp Dictionary", insert: "<YtypDefinitionDictionary schema=\"newengine.ytyp.definition_dictionary.v1\" representation=\"xml\" body_format=\"newengine.xml.metadata.v1\">\n  <Entry name=\"entry_name\" kind=\"game_ready_metadata\" entry_kind=\"archetype_definition\">\n  </Entry>\n</YtypDefinitionDictionary>\n", detail: "Root .ytyp Definition Entry dictionary" },
];
static DEFINITION_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Entry", insert: "\n  <Entry name=\"entry_name\" kind=\"game_ready_metadata\" entry_kind=\"archetype_definition\">\n    <Dependencies>\n    </Dependencies>\n  </Entry>", detail: "Addressable Definition Entry" },
    XmlSnippet { label: "Dependency", insert: "\n      <Dependency domain=\"engine.assets.models\" reference=\"path/file.ydd@entry\" role=\"resource\" required=\"true\" />", detail: "Typed dependency edge" },
    XmlSnippet { label: "Namespace", insert: "\n    <Metadata>\n      <Namespace name=\"newengine.game_ready\">\n      </Namespace>\n    </Metadata>", detail: "Domain metadata namespace" },
];
static MAP_ROOT_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Ymap Map", insert: "<YmapMapDefinition schema=\"newengine.map.definition.v1\" representation=\"xml\" body_format=\"newengine.xml.metadata.v1\">\n  <map name=\"map_name\">\n    <definition_refs>\n    </definition_refs>\n    <placements>\n    </placements>\n  </map>\n</YmapMapDefinition>\n", detail: "Root authored map definition" },
];
static MAP_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "DefinitionRef", insert: "\n      <DefinitionRef value=\"world/file.ytyp@entry\" />", detail: "Referenced Definition Entry" },
    XmlSnippet { label: "Placement", insert: "\n      <Placement definition_ref=\"world/file.ytyp@entry\" position=\"0,0,0\" rotation_ypr=\"0,0,0\" scale=\"1,1,1\" apply_mode=\"metadata_only\" />", detail: "Map placement/apply command" },
];
static MATERIAL_ROOT_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Nemat Library", insert: "<NematMaterialLibrary schema=\"newengine.nemat.material_library.v1\" version=\"1\" representation=\"xml\" body_format=\"newengine.xml.metadata.v1\">\n  <Material name=\"material_name\" shader=\"pbr.default\">\n    <Surface blend=\"opaque\" two_sided=\"false\" />\n    <Textures>\n    </Textures>\n    <Params>\n    </Params>\n  </Material>\n</NematMaterialLibrary>\n", detail: "Root .nemat material library" },
];
static MATERIAL_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Material", insert: "\n  <Material name=\"material_name\" shader=\"pbr.default\">\n    <Surface blend=\"opaque\" two_sided=\"false\" />\n    <Textures>\n    </Textures>\n    <Params>\n    </Params>\n  </Material>", detail: "Addressable .nemat@entry material" },
    XmlSnippet { label: "Texture", insert: "\n      <Texture slot=\"base_color\" ref=\"textures/file.ytd@entry\" />", detail: "Material texture slot -> .ytd@entry" },
    XmlSnippet { label: "Param", insert: "\n      <Param name=\"roughness\" type=\"float\" value=\"0.8\" />", detail: "Typed material parameter" },
];
static METADATA_ROOT_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Ymt Metadata", insert: "<YmtMetadata schema=\"newengine.ymt.metadata.v1\" representation=\"xml\" body_format=\"newengine.xml.metadata.v1\">\n  <Entry name=\"metadata_entry\">\n  </Entry>\n</YmtMetadata>\n", detail: "Root .ymt metadata container" },
];
static METADATA_CHILD_SNIPPETS: &[XmlSnippet] = DEFINITION_CHILD_SNIPPETS;
static NEUI_ROOT_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "NeUi Surface Dictionary", insert: r##"<NeUiDictionary schema="newengine.neui.dictionary.v1" representation="xmlcentral" owner_scope="engine" document_kind="surface">
  <Surface name="engine.ui.loading" root="layout.main" theme="assets/ui/themes/north_star_dark.neui@theme" bindings="bindings">
    <Dependencies>
    </Dependencies>
  </Surface>

  <Layout name="layout.main" surface="engine.ui.loading">
    <Panel id="root" class="surface-shell" />
  </Layout>

  <BindingGraph name="bindings">
  </BindingGraph>
</NeUiDictionary>
"##, detail: "Root .neui surface dictionary" },
    XmlSnippet { label: "NeUi Registry", insert: r##"<NeUiRegistry schema="newengine.neui.registry.v1">
  <Surfaces>
    <SurfaceRef id="engine.ui.loading" ref="assets/ui/engine/loading.neui@surface" />
  </Surfaces>
  <Themes>
    <ThemeRef id="north_star.dark" ref="assets/ui/themes/north_star_dark.neui@theme" />
  </Themes>
  <ComponentPacks>
  </ComponentPacks>
</NeUiRegistry>
"##, detail: "Registry of UI refs only; no inline layouts" },
    XmlSnippet { label: "NeUi Theme Library", insert: r##"<NeUiThemeLibrary schema="newengine.neui.theme.v1" representation="xmlcentral" owner_scope="shared" document_kind="theme">
  <Theme name="north_star.dark">
    <Token name="color.bg" value="#0B0D10" />
    <Token name="color.accent" value="#FF7A18" />
  </Theme>
</NeUiThemeLibrary>
"##, detail: "Theme tokens split from surfaces" },
];
static NEUI_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet { label: "Surface", insert: r##"
  <Surface name="engine.ui.loading" root="layout.main" theme="assets/ui/themes/north_star_dark.neui@theme" bindings="bindings">
    <Dependencies>
      <ComponentRef ref="assets/ui/components/cards.neui@card.status" />
      <TextureRef ref="assets/ui/icons/builtin_icons.ytd@app_logo" />
    </Dependencies>
  </Surface>"##, detail: "Addressable UI surface entry" },
    XmlSnippet { label: "Layout", insert: r##"
  <Layout name="layout.main" surface="engine.ui.loading">
    <Panel id="root" class="surface-shell" />
  </Layout>"##, detail: "UI layout tree" },
    XmlSnippet { label: "BindingGraph", insert: r##"
  <BindingGraph name="bindings">
    <StateSource id="loading" source="engine.ui.loading.status" contract="LoadingStatusSnapshot" update="event" />
    <Bind element="loading.progress" property="value" source="loading.progress" />
  </BindingGraph>"##, detail: "Declarative state binding plan" },
    XmlSnippet { label: "ActionMap", insert: r##"
  <ActionMap name="actions">
    <Action id="game.resume" target="engine.lifecycle" command="game.resume" />
  </ActionMap>"##, detail: "UI actions routed through engine gateway contracts" },
    XmlSnippet { label: "ComponentRef", insert: r##"
      <ComponentRef ref="assets/ui/components/buttons.neui@button.primary" />"##, detail: "Reference reusable component entry" },
];
pub const NEUI_ROOT_NAMES: &[&str] = &[
    "NeUiDictionary",
    "NeUiRegistry",
    "NeUiThemeLibrary",
    "NeUiComponentLibrary",
    "NeUiBindingLibrary",
];

#[inline]
pub fn is_neui_root_name(name: &str) -> bool { NEUI_ROOT_NAMES.iter().any(|candidate| *candidate == name) }

static EMPTY_SNIPPETS: &[XmlSnippet] = &[];

pub fn completion_catalog_for_extension(extension: &str) -> XmlCompletionCatalog {
    match extension.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "ytyp" => XmlCompletionCatalog { schema_family: "newengine.ytyp.definition_dictionary.v1", root_snippets: DEFINITION_ROOT_SNIPPETS, child_snippets: DEFINITION_CHILD_SNIPPETS },
        "ymap" => XmlCompletionCatalog { schema_family: "newengine.map.definition.v1", root_snippets: MAP_ROOT_SNIPPETS, child_snippets: MAP_CHILD_SNIPPETS },
        "ymt" => XmlCompletionCatalog { schema_family: "newengine.ymt.metadata.v1", root_snippets: METADATA_ROOT_SNIPPETS, child_snippets: METADATA_CHILD_SNIPPETS },
        "nemat" => XmlCompletionCatalog { schema_family: "newengine.nemat.material_library.v1", root_snippets: MATERIAL_ROOT_SNIPPETS, child_snippets: MATERIAL_CHILD_SNIPPETS },
        "neui" => XmlCompletionCatalog { schema_family: "newengine.neui.dictionary.v1", root_snippets: NEUI_ROOT_SNIPPETS, child_snippets: NEUI_CHILD_SNIPPETS },
        _ => XmlCompletionCatalog { schema_family: "generic.xml", root_snippets: EMPTY_SNIPPETS, child_snippets: EMPTY_SNIPPETS },
    }
}

pub fn completion_catalog_for_text_or_extension(text: &str, extension: &str) -> XmlCompletionCatalog {
    if let Ok(doc) = parse_xml_document(text, "completion_catalog") {
        let root = doc.root_element();
        match root.tag_name().name() {
            "YtypDefinitionDictionary" | "DefinitionDictionary" => return completion_catalog_for_extension("ytyp"),
            "YmapMapDefinition" | "MapDefinition" => return completion_catalog_for_extension("ymap"),
            "YmtMetadata" => return completion_catalog_for_extension("ymt"),
            "NematMaterialLibrary" | "MaterialLibrary" => return completion_catalog_for_extension("nemat"),
            name if is_neui_root_name(name) => return completion_catalog_for_extension("neui"),
            _ => {}
        }
    }
    completion_catalog_for_extension(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_scalar_keeps_comma_prose_as_string() {
        assert_eq!(
            xml_scalar("Walk data-driven ytyp, ydd, nemat and ytd assets."),
            serde_json::Value::String("Walk data-driven ytyp, ydd, nemat and ytd assets.".to_owned())
        );
    }

    #[test]
    fn xml_scalar_parses_numeric_vectors() {
        assert_eq!(xml_scalar("1.0,2.5,-3.0"), serde_json::json!([1.0, 2.5, -3.0]));
    }

    #[test]
    fn neui_catalog_recognizes_dictionary_root() {
        let catalog = completion_catalog_for_text_or_extension(
            "<NeUiDictionary schema=\"newengine.neui.dictionary.v1\" />",
            "xml",
        );
        assert_eq!(catalog.schema_family, "newengine.neui.dictionary.v1");
        assert!(catalog.root_snippets.iter().any(|snippet| snippet.label.contains("NeUi")));
    }
}
