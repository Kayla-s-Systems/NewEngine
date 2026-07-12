#![forbid(unsafe_op_in_unsafe_fn)]

//! Central authored XML helpers for NEF8/ListFile metadata bodies.
//!
//! This crate intentionally owns only presentation-level XML mechanics:
//! UTF-8/XML detection, stable tree-to-value projection, formatting and editor
//! completion hints. Domain semantics remain in `engine.assets.definitions`,
//! `engine.scene`, `engine.assets.materials`, etc.

mod completion;
mod document;
mod formatter;
mod projection;
mod query;

pub use completion::{
    completion_catalog_for_extension, completion_catalog_for_text_or_extension, is_neui_root_name,
    XmlCompletionCatalog, XmlSnippet, NEUI_ROOT_NAMES,
};
pub use document::{
    body_is_xml, parse_xml_body, parse_xml_document, text_is_xml, XmlDocument, XmlNode,
};
pub use formatter::format_xml_lossy;
pub use projection::{
    xml_insert_child, xml_namespace_map, xml_node_children_object, xml_node_object, xml_scalar,
    xml_to_json_projection,
};
pub use query::{
    root_has_any_name, root_schema, xml_attr_any, xml_attr_bool_any, xml_attr_f32_any,
    xml_attr_u32_any, xml_attr_u64_any, xml_child, xml_children_named, xml_tags,
};
