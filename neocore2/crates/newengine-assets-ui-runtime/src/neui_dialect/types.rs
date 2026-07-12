use super::catalog::{kind_for_tag, METADATA_ELEMENTS, STRUCTURAL_ATTRS};
use super::helpers::*;
use super::*;

/// Static NEUI dialect registry.
///
/// This is the only place where XML tag aliases, metadata elements and
/// structural attributes are declared. The compiler pipeline consumes this
/// registry; it must not grow ad-hoc per-screen or per-game branches.
/// A later step can replace this static table with a loaded `.neui.schema` asset.
pub(crate) const DEFAULT_NEUI_DIALECT_REF: &str = "ui/dialects/runtime.neui@dialect";

#[derive(Debug, Clone)]
pub(crate) struct NeUiDialect {
    pub id: String,
    metadata_elements: Vec<String>,
    structural_attrs: Vec<String>,
    tag_rules: Vec<NeUiRuntimeTagRule>,
    interactive_kinds: Vec<UiRuntimeNodeKind>,
}

#[derive(Debug, Clone)]
struct NeUiRuntimeTagRule {
    aliases: Vec<String>,
    kind: UiRuntimeNodeKind,
    implicit_tags: Vec<String>,
    add_source_tag: bool,
    add_normalized_tag: bool,
}

impl NeUiDialect {
    pub(crate) fn builtin() -> Self {
        Self {
            id: "newengine.neui.dialect.runtime.builtin.v1".to_owned(),
            metadata_elements: METADATA_ELEMENTS
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
            structural_attrs: STRUCTURAL_ATTRS.iter().map(|it| (*it).to_owned()).collect(),
            tag_rules: Vec::new(),
            interactive_kinds: vec![
                UiRuntimeNodeKind::Action,
                UiRuntimeNodeKind::Button,
                UiRuntimeNodeKind::Input,
                UiRuntimeNodeKind::Checkbox,
                UiRuntimeNodeKind::Toggle,
                UiRuntimeNodeKind::Slider,
                UiRuntimeNodeKind::ScrollBar,
                UiRuntimeNodeKind::Select,
                UiRuntimeNodeKind::List,
                UiRuntimeNodeKind::Tree,
                UiRuntimeNodeKind::Split,
                UiRuntimeNodeKind::Viewport,
            ],
        }
    }

    pub(crate) fn from_xml(xml: &str, source_ref: &str) -> Result<Self, String> {
        let root = first_element(xml, "NeUiDialect")
            .ok_or_else(|| format!("{source_ref}: dialect asset has no <NeUiDialect> root"))?;
        let mut dialect = Self::builtin();
        dialect.id = attr_value(&root.open, "id")
            .or_else(|| attr_value(&root.open, "name"))
            .unwrap_or_else(|| source_ref.to_owned());

        if let Some(metadata) = first_element(&root.inner, "Metadata") {
            let mut values = Vec::new();
            for item in direct_child_elements(&metadata.inner) {
                if let Some(name) =
                    attr_value(&item.open, "name").or_else(|| attr_value(&item.open, "tag"))
                {
                    push_unique(&mut values, normalize_name(&name));
                }
            }
            if !values.is_empty() {
                dialect.metadata_elements = values;
            }
        }
        if let Some(attrs) = first_element(&root.inner, "StructuralAttrs")
            .or_else(|| first_element(&root.inner, "StructuralAttributes"))
        {
            let mut values = Vec::new();
            for item in direct_child_elements(&attrs.inner) {
                if let Some(name) =
                    attr_value(&item.open, "name").or_else(|| attr_value(&item.open, "attr"))
                {
                    push_unique(&mut values, name.trim().to_owned());
                }
            }
            if !values.is_empty() {
                dialect.structural_attrs = values;
            }
        }
        if let Some(tags) = first_element(&root.inner, "Tags") {
            for item in direct_child_elements(&tags.inner) {
                if !matches!(item.name.as_str(), "Tag" | "Element" | "Node") {
                    continue;
                }
                let aliases = split_tokens(
                    attr_value(&item.open, "aliases")
                        .or_else(|| attr_value(&item.open, "alias"))
                        .or_else(|| attr_value(&item.open, "names"))
                        .or_else(|| attr_value(&item.open, "name"))
                        .unwrap_or_default()
                        .as_str(),
                )
                .into_iter()
                .map(|it| normalize_name(&it))
                .filter(|it| !it.is_empty())
                .collect::<Vec<_>>();
                if aliases.is_empty() {
                    continue;
                }
                let kind_text = attr_value(&item.open, "kind")
                    .or_else(|| attr_value(&item.open, "node_kind"))
                    .unwrap_or_else(|| "Panel".to_owned());
                let kind = node_kind_from_str(&kind_text).map_err(|err| {
                    format!("{source_ref}: dialect tag aliases={aliases:?} {err}")
                })?;
                let implicit_tags = split_tokens(
                    attr_value(&item.open, "implicit_tags")
                        .or_else(|| attr_value(&item.open, "tags"))
                        .unwrap_or_default()
                        .as_str(),
                )
                .into_iter()
                .map(|it| sanitize_tag(&it))
                .filter(|it| !it.is_empty())
                .collect::<Vec<_>>();
                dialect.tag_rules.push(NeUiRuntimeTagRule {
                    aliases,
                    kind,
                    implicit_tags,
                    add_source_tag: attr_bool(&item.open, "add_source_tag").unwrap_or(false),
                    add_normalized_tag: attr_bool(&item.open, "add_normalized_tag")
                        .unwrap_or(false),
                });
            }
        }
        if let Some(kinds) = first_element(&root.inner, "InteractiveKinds") {
            let mut values = Vec::new();
            for item in direct_child_elements(&kinds.inner) {
                if let Some(name) =
                    attr_value(&item.open, "name").or_else(|| attr_value(&item.open, "kind"))
                {
                    let kind = node_kind_from_str(&name)
                        .map_err(|err| format!("{source_ref}: interactive kind '{name}' {err}"))?;
                    if !values.contains(&kind) {
                        values.push(kind);
                    }
                }
            }
            if !values.is_empty() {
                dialect.interactive_kinds = values;
            }
        }
        Ok(dialect)
    }

    pub(crate) fn is_metadata_element(&self, name: &str) -> bool {
        let normalized = normalize_name(name);
        self.metadata_elements.iter().any(|it| it == &normalized)
    }

    pub(crate) fn kind_for_tag(&self, name: &str) -> (UiRuntimeNodeKind, Vec<String>) {
        let normalized = normalize_name(name);
        let source_tag = sanitize_tag(name);
        for rule in &self.tag_rules {
            if rule.aliases.iter().any(|alias| alias == &normalized) {
                let mut tags = rule.implicit_tags.clone();
                if rule.add_source_tag && !source_tag.is_empty() {
                    tags.push(source_tag.clone());
                }
                if rule.add_normalized_tag && !normalized.is_empty() {
                    tags.push(normalized.clone());
                }
                tags.sort();
                tags.dedup();
                return (rule.kind, tags);
            }
        }
        kind_for_tag(name)
    }

    pub(crate) fn is_structural_attr(&self, name: &str) -> bool {
        self.structural_attrs.iter().any(|it| it == name.trim())
    }

    pub(crate) fn is_intrinsically_interactive(&self, kind: UiRuntimeNodeKind) -> bool {
        self.interactive_kinds.contains(&kind)
    }

    pub(crate) fn inspect_json(
        &self,
        dialect_ref: &str,
        source: &str,
        warnings: Vec<String>,
    ) -> serde_json::Value {
        serde_json::json!({
            "ok": true,
            "schema": "newengine.assets.ui.dialect.inspect.response.v1",
            "dialect_ref": dialect_ref,
            "dialect_id": self.id,
            "source": source,
            "metadata_elements": self.metadata_elements,
            "structural_attrs": self.structural_attrs,
            "interactive_kinds": self.interactive_kinds,
            "tag_rules": self.tag_rules.iter().map(|rule| serde_json::json!({
                "aliases": rule.aliases,
                "kind": rule.kind,
                "implicit_tags": rule.implicit_tags,
                "add_source_tag": rule.add_source_tag,
                "add_normalized_tag": rule.add_normalized_tag,
            })).collect::<Vec<_>>(),
            "warnings": warnings,
        })
    }
}
