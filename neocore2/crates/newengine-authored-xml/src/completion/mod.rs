mod definition;
mod map;
mod material;
mod metadata;
mod neui;

use crate::document::parse_xml_document;
use definition::{DEFINITION_CHILD_SNIPPETS, DEFINITION_ROOT_SNIPPETS};
use map::{MAP_CHILD_SNIPPETS, MAP_ROOT_SNIPPETS};
use material::{MATERIAL_CHILD_SNIPPETS, MATERIAL_ROOT_SNIPPETS};
use metadata::METADATA_ROOT_SNIPPETS;
use neui::{NEUI_CHILD_SNIPPETS, NEUI_ROOT_SNIPPETS};

pub use neui::{is_neui_root_name, NEUI_ROOT_NAMES};

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

static EMPTY_SNIPPETS: &[XmlSnippet] = &[];

pub fn completion_catalog_for_extension(extension: &str) -> XmlCompletionCatalog {
    match extension
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "ytyp" => catalog(
            "newengine.ytyp.properties.v1",
            DEFINITION_ROOT_SNIPPETS,
            DEFINITION_CHILD_SNIPPETS,
        ),
        "ymap" => catalog(
            "newengine.map.definition.v1",
            MAP_ROOT_SNIPPETS,
            MAP_CHILD_SNIPPETS,
        ),
        "ymt" => catalog(
            "newengine.ymt.metadata.v1",
            METADATA_ROOT_SNIPPETS,
            DEFINITION_CHILD_SNIPPETS,
        ),
        "nemat" => catalog(
            "newengine.nemat.material_library.v1",
            MATERIAL_ROOT_SNIPPETS,
            MATERIAL_CHILD_SNIPPETS,
        ),
        "neui" => catalog(
            "newengine.neui.dictionary.v1",
            NEUI_ROOT_SNIPPETS,
            NEUI_CHILD_SNIPPETS,
        ),
        _ => catalog("generic.xml", EMPTY_SNIPPETS, EMPTY_SNIPPETS),
    }
}

pub fn completion_catalog_for_text_or_extension(
    text: &str,
    extension: &str,
) -> XmlCompletionCatalog {
    if let Ok(doc) = parse_xml_document(text, "completion_catalog") {
        let root_name = doc.root_element().tag_name().name();

        let detected_extension = match root_name {
            "YtypProperties" | "AssetProperties" => Some("ytyp"),
            "YmapMapDefinition" | "MapDefinition" => Some("ymap"),
            "YmtMetadata" => Some("ymt"),
            "NematMaterialLibrary" | "MaterialLibrary" => Some("nemat"),
            name if is_neui_root_name(name) => Some("neui"),
            _ => None,
        };

        if let Some(detected_extension) = detected_extension {
            return completion_catalog_for_extension(detected_extension);
        }
    }

    completion_catalog_for_extension(extension)
}

const fn catalog(
    schema_family: &'static str,
    root_snippets: &'static [XmlSnippet],
    child_snippets: &'static [XmlSnippet],
) -> XmlCompletionCatalog {
    XmlCompletionCatalog {
        schema_family,
        root_snippets,
        child_snippets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neui_catalog_recognizes_dictionary_root() {
        let catalog = completion_catalog_for_text_or_extension(
            "<NeUiDictionary schema=\"newengine.neui.dictionary.v1\" />",
            "xml",
        );

        assert_eq!(catalog.schema_family, "newengine.neui.dictionary.v1");
        assert!(catalog
            .root_snippets
            .iter()
            .any(|snippet| snippet.label.contains("NeUi")));
    }

    #[test]
    fn extension_matching_is_case_insensitive_and_accepts_dot() {
        let catalog = completion_catalog_for_extension(".YMAP");

        assert_eq!(catalog.schema_family, "newengine.map.definition.v1");
    }
}
