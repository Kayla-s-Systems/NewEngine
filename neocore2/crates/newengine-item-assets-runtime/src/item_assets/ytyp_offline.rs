use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{ytyp::apply_weapon_ytyp_namespace, AuthoredItemPackage};

fn normalized_definition_path(reference: &str) -> Option<String> {
    let raw = reference.trim().replace('\\', "/");
    let path = raw
        .split_once('@')
        .map(|(path, _)| path)
        .unwrap_or(raw.as_str())
        .trim_matches('/');
    (!path.is_empty()).then(|| path.to_owned())
}

fn source_candidates(root: &Path, reference: &str) -> Vec<PathBuf> {
    let Some(path) = normalized_definition_path(reference) else {
        return Vec::new();
    };
    let variants = if let Some(stripped) = path.strip_prefix("shared/") {
        vec![stripped.to_owned(), path]
    } else {
        vec![path]
    };
    let mut out = Vec::new();
    for relative in variants {
        let relative = PathBuf::from(relative);
        let file_name = relative
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let stem = file_name.strip_suffix(".ytyp").unwrap_or(file_name);
        for base in [root.join("Source"), root.to_path_buf()] {
            out.push(base.join(&relative).with_extension("ytyp.xml"));
            out.push(base.join(&relative).join(format!("{stem}.ytyp.xml")));
        }
    }
    out
}

fn read_weapon_namespace(path: &Path) -> Result<serde_json::Value, String> {
    let body = fs::read(path)
        .map_err(|error| format!("read YTYP source '{}' failed: {error}", path.display()))?;
    let source = path.display().to_string();
    let document = newengine_authored_xml::parse_xml_body(&body, &source)?;
    let root = document.root_element();
    let metadata = newengine_authored_xml::xml_child(root, "Metadata")
        .ok_or_else(|| format!("YTYP '{}' has no Metadata", path.display()))?;
    for namespace in newengine_authored_xml::xml_children_named(metadata, "Namespace") {
        let name = newengine_authored_xml::xml_attr_any(namespace, &["name", "namespace"])
            .unwrap_or_default();
        if name.trim().eq_ignore_ascii_case("newengine.weapon") {
            return Ok(newengine_authored_xml::xml_node_children_object(namespace));
        }
    }
    Err(format!(
        "YTYP '{}' has no newengine.weapon namespace",
        path.display()
    ))
}

pub fn hydrate_item_package_from_ytyp_source_roots(
    package: &mut AuthoredItemPackage,
    source_roots: &[PathBuf],
) -> Result<usize, String> {
    let mut hydrated = 0usize;
    for item in &mut package.items {
        let reference = item.definition_ref.trim().to_owned();
        if reference.is_empty() {
            continue;
        }

        let mut resolved = None;
        let mut attempted = Vec::new();
        for root in source_roots {
            for candidate in source_candidates(root, &reference) {
                if !attempted.contains(&candidate) {
                    attempted.push(candidate.clone());
                }
                if candidate.is_file() {
                    resolved = Some(candidate);
                    break;
                }
            }
            if resolved.is_some() {
                break;
            }
        }

        let path = resolved.ok_or_else(|| {
            format!(
                "offline YTYP source unavailable item='{}' ref='{}' attempted=[{}]",
                item.id,
                reference,
                attempted
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        let namespace = read_weapon_namespace(&path)?;
        apply_weapon_ytyp_namespace(item, &namespace).map_err(|error| {
            format!(
                "offline YTYP hydration failed item='{}' ref='{}' source='{}': {error}",
                item.id,
                reference,
                path.display()
            )
        })?;
        hydrated += 1;
    }
    Ok(hydrated)
}
