use super::*;

pub(super) fn extension_of_ref(reference: &str) -> Option<String> {
    let (path, _) = split_asset_ref(reference);
    path.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
}

pub(super) fn refs_to_edges(
    mut refs: Vec<String>,
    default_role: &str,
) -> Vec<(String, String, bool)> {
    refs.sort();
    refs.dedup();
    refs.into_iter()
        .map(|reference| {
            let role = match extension_of_ref(&reference).as_deref() {
                Some("ydd") => "drawable_dictionary",
                Some("nemat") => "material_library",
                Some("ytd") => "texture_dictionary",
                Some("ybn") | Some("ycol") => "physics_dictionary",
                Some("nebrain") => "ai_brain",
                Some("nepat") => "ai_pattern",
                Some("nemem") => "ai_memory",
                Some("ytyp") => "model_properties_descriptor",
                _ => default_role,
            };
            (reference, role.to_owned(), true)
        })
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
    let lower = value.to_ascii_lowercase();
    [
        ".ytyp",
        ".ydd@",
        ".ytyd@",
        ".ydr@",
        ".yft@",
        ".nemat@",
        ".ytd@",
        ".ymap@",
        ".ymf@",
        ".ymt@",
        ".ybn@",
        ".ybd@",
        ".ycol@",
        ".ycd@",
        ".yed@",
        ".yfd@",
        ".yld@",
        ".ypdb@",
        ".yvr@",
        ".ywr@",
        ".ysc",
        ".ytf@",
        ".nebrain@",
        ".nepat@",
        ".nemem@",
        ".negoal@",
        ".nebt@",
        ".nebehavior@",
        ".neutility@",
        ".nebb@",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
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
