use super::*;

pub(super) fn refs_to_edges(
    mut refs: Vec<String>,
    default_role: &str,
) -> Vec<(String, String, bool)> {
    refs.sort();
    refs.dedup();
    refs.into_iter()
        .map(|reference| (reference, default_role.to_owned(), true))
        .collect()
}

pub(super) fn definition_entry_refs_to_edges(
    refs_value: Option<&serde_json::Value>,
    owner_ref: &str,
) -> Vec<(String, String, bool)> {
    let Some(refs_value) = refs_value.and_then(|value| value.as_object()) else {
        return Vec::new();
    };
    let mut edges = Vec::new();
    for (field, role) in [
        ("drawable_refs", "definition/drawable_dependency"),
        ("material_refs", "definition/material_dependency"),
        ("texture_refs", "definition/texture_dependency"),
        ("uv_layout_refs", "definition/uv_layout_dependency"),
        ("physics_refs", "definition/physics_dependency"),
        ("collision_refs", "definition/collision_dependency"),
        ("ai_refs", "definition/ai_dependency"),
        ("streaming_refs", "definition/streaming_dependency"),
        ("editor_refs", "definition/editor_dependency"),
        ("other_refs", "definition/other_dependency"),
    ] {
        let Some(items) = refs_value.get(field).and_then(|value| value.as_array()) else {
            continue;
        };
        for item in items {
            let Some(text) = item.as_str() else {
                continue;
            };
            let reference = normalize_asset_ref(text);
            if reference.is_empty() || reference == owner_ref {
                continue;
            }
            edges.push((reference, role.to_owned(), true));
        }
    }
    edges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    edges.dedup();
    edges
}

pub(super) fn list_file_manifest_dependency_edges(
    value: &serde_json::Value,
    default_role: &str,
) -> Vec<(String, String, bool)> {
    fn collect_array(
        value: Option<&serde_json::Value>,
        default_role: &str,
        out: &mut Vec<(String, String, bool)>,
    ) {
        let Some(items) = value.and_then(serde_json::Value::as_array) else {
            return;
        };
        for item in items {
            let Some(object) = item.as_object() else {
                continue;
            };
            let Some(reference) = object
                .get("reference")
                .and_then(serde_json::Value::as_str)
                .map(normalize_asset_ref)
                .filter(|reference| !reference.is_empty())
            else {
                continue;
            };
            let role = object
                .get("role")
                .or_else(|| object.get("kind"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|role| !role.is_empty())
                .unwrap_or(default_role)
                .to_owned();
            let required = object
                .get("required")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            out.push((reference, role, required));
        }
    }

    let mut edges = Vec::new();
    collect_array(value.get("dependencies"), default_role, &mut edges);
    if let Some(entries) = value.get("entries").and_then(serde_json::Value::as_array) {
        for entry in entries {
            collect_array(entry.get("dependencies"), default_role, &mut edges);
        }
    }
    edges.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    edges.dedup();
    edges
}

pub(super) fn collect_ref_strings(value: &serde_json::Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_ref_strings_into(value, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect_ref_strings_into(value: &serde_json::Value, refs: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            let normalized = normalize_asset_ref(text);
            if looks_like_runtime_asset_ref(&normalized) {
                refs.push(normalized);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ref_strings_into(item, refs);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_ref_strings_into(value, refs);
            }
        }
        _ => {}
    }
}

fn looks_like_runtime_asset_ref(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    let Ok(reference) = newengine_assets_api::parse_asset_reference(value) else {
        return false;
    };
    // Candidate detection is intentionally syntax-only. Whether the suffix is a
    // registered asset type is decided later by engine.assets.types.
    reference.logical_path.rsplit_once('.').is_some()
        && (value.contains('/') || value.contains('@'))
}

pub(super) fn collect_metadata_namespaces(
    graph: &mut ResolvedAssetGraphV2,
    owner_ref: &str,
    value: &serde_json::Value,
) {
    if let Some(namespaces) = value
        .get("metadata_namespaces")
        .or_else(|| value.get("metadata"))
        .and_then(|v| v.as_array())
    {
        for namespace in namespaces {
            if let Some(name) = namespace
                .get("namespace")
                .or_else(|| namespace.get("name"))
                .and_then(|v| v.as_str())
            {
                attach_metadata_namespace(graph, owner_ref, name);
            }
        }
    }
    if let Some(side_effects) = value.get("side_effects").and_then(|v| v.as_object()) {
        for key in side_effects.keys() {
            attach_metadata_namespace(graph, owner_ref, format!("side_effect:{key}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listfile_dependencies_do_not_scrape_policy_strings() {
        let manifest = serde_json::json!({
            "dependencies": [{
                "reference": "models/characters/abby/abby.ymt@abby",
                "role": "skeleton",
                "required": true
            }],
            "entries": [{
                "dependencies": [{
                    "reference": "animations/characters/abby/idle.ycd@idle",
                    "role": "animation/idle",
                    "required": true
                }]
            }],
            "policy": [
                "YCD entries are addressed as file.ycd@clip",
                "documentation may mention textures/example.ytd@entry without declaring a dependency"
            ],
            "warnings": ["missing demo/example.ydd@mesh is only diagnostic text"]
        });
        let edges = list_file_manifest_dependency_edges(&manifest, "listfile_dependency");
        assert_eq!(edges.len(), 2);
        assert!(edges
            .iter()
            .any(|edge| edge.0 == "models/characters/abby/abby.ymt@abby"));
        assert!(edges
            .iter()
            .any(|edge| edge.0 == "animations/characters/abby/idle.ycd@idle"));
        assert!(!edges.iter().any(|edge| edge.0.contains("file.ycd")));
        assert!(!edges.iter().any(|edge| edge.0.contains("example.ytd")));
        assert!(!edges.iter().any(|edge| edge.0.contains("example.ydd")));
    }
}
