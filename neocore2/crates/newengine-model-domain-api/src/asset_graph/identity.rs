use super::*;

pub fn cache_key_parts_for_ref(reference: &str) -> AssetGraphCacheKeyParts {
    let (logical_path, entry) = split_ref(reference);
    AssetGraphCacheKeyParts {
        logical_path,
        entry,
        schema_version: "v2".to_owned(),
        ..Default::default()
    }
}

pub(super) fn stable_graph_cache_key(graph: &ResolvedAssetGraphV2) -> String {
    let mut key = format!("root={}|schema={}", graph.root_ref, graph.schema);
    for node in &graph.nodes {
        key.push_str("|node=");
        key.push_str(&node.reference);
        key.push(':');
        key.push_str(node.content_hash.as_deref().unwrap_or(""));
        key.push(':');
        key.push_str(node.entry_hash.as_deref().unwrap_or(""));
        key.push(':');
        key.push_str(&node.schema_version);
    }
    for edge in &graph.edges {
        key.push_str("|edge=");
        key.push_str(&edge.from_ref);
        key.push_str("->");
        key.push_str(&edge.to_ref);
        key.push(':');
        key.push_str(&edge.kind);
    }
    format!("asset-graph-v2:{:016x}", fnv1a64(key.as_bytes()))
}

pub fn split_asset_ref(reference: &str) -> (String, Option<String>) {
    split_ref(reference)
}

pub(super) fn split_ref(reference: &str) -> (String, Option<String>) {
    let normalized = normalize_asset_ref(reference);
    match normalized.rsplit_once('@') {
        Some((path, entry)) if !entry.trim().is_empty() => {
            (path.to_owned(), Some(entry.trim().to_owned()))
        }
        _ => (normalized, None),
    }
}

pub fn normalize_asset_ref(reference: &str) -> String {
    let mut s = reference.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_owned();
    }
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    s.trim_start_matches('/').to_owned()
}

pub fn stable_graph_id(reference: &str) -> String {
    let normalized = normalize_asset_ref(reference);
    format!("asset:{:016x}", fnv1a64(normalized.as_bytes()))
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
