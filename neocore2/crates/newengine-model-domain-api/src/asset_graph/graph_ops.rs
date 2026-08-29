use super::identity::{split_ref, stable_graph_cache_key};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub fn push_manifest_dependency(
    graph: &mut ResolvedAssetGraphV2,
    owner_ref: &str,
    reference: &str,
    role: &str,
    required: bool,
) {
    let reference = normalize_asset_ref(reference);
    if reference.trim().is_empty() {
        graph.metadata_warnings.push(format!(
            "empty dependency reference from '{owner_ref}' role='{role}'"
        ));
        return;
    }
    let (node_role, asset_kind, gateway, method) = classify_ref(&reference);
    push_node(graph, &reference, node_role, asset_kind, gateway, method);
    push_edge(graph, owner_ref, &reference, role, required);
}

pub fn attach_vfs_source(
    graph: &mut ResolvedAssetGraphV2,
    reference: &str,
    source: AssetGraphVfsSource,
) {
    let id = stable_graph_id(reference);
    for node in &mut graph.nodes {
        if node.id == id {
            node.vfs_source = source.clone();
        }
    }
}

pub fn attach_content_hash(
    graph: &mut ResolvedAssetGraphV2,
    reference: &str,
    content_hash: impl Into<String>,
) {
    let id = stable_graph_id(reference);
    let content_hash = content_hash.into();
    for node in &mut graph.nodes {
        if node.id == id {
            node.content_hash = Some(content_hash.clone());
            node.cache_key_parts.content_hash = Some(content_hash.clone());
        }
    }
}

pub fn attach_node_warning(
    graph: &mut ResolvedAssetGraphV2,
    reference: &str,
    warning: impl Into<String>,
) {
    let id = stable_graph_id(reference);
    let warning = warning.into();
    for node in &mut graph.nodes {
        if node.id == id {
            node.warnings.push(warning.clone());
        }
    }
}

pub fn attach_metadata_namespace(
    graph: &mut ResolvedAssetGraphV2,
    reference: &str,
    namespace: impl Into<String>,
) {
    let id = stable_graph_id(reference);
    let namespace = namespace.into();
    for node in &mut graph.nodes {
        if node.id == id && !node.metadata_namespaces.contains(&namespace) {
            node.metadata_namespaces.push(namespace.clone());
        }
    }
}

pub fn finalize_graph(graph: &mut ResolvedAssetGraphV2) {
    for node in &mut graph.nodes {
        node.reference = normalize_asset_ref(&node.reference);
        node.asset_ref = node.reference.clone();
        node.kind = if node.kind.trim().is_empty() {
            node.asset_kind.clone()
        } else {
            node.kind.clone()
        };
        node.entry_hash = split_ref(&node.reference)
            .1
            .map(|entry| format!("fnv1a64:{:016x}", fnv1a64(entry.as_bytes())));
        node.schema_version = if node.schema_version.trim().is_empty() {
            "v2".to_owned()
        } else {
            node.schema_version.clone()
        };
        node.cache_key_parts = cache_key_parts_for_ref(&node.reference);
        node.cache_key_parts.content_hash = node.content_hash.clone();
        node.cache_key_parts.schema_version = node.schema_version.clone();
        node.warnings.sort();
        node.warnings.dedup();
        node.metadata_namespaces.sort();
        node.metadata_namespaces.dedup();
    }
    graph.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    graph.nodes.dedup_by(|a, b| a.id == b.id);
    graph.edges.sort_by(|a, b| {
        (a.from.as_str(), a.to.as_str(), a.kind.as_str()).cmp(&(
            b.from.as_str(),
            b.to.as_str(),
            b.kind.as_str(),
        ))
    });
    graph
        .edges
        .dedup_by(|a, b| a.from == b.from && a.to == b.to && a.kind == b.kind);
    graph.missing_refs.sort();
    graph.missing_refs.dedup();
    graph.format_warnings.sort();
    graph.format_warnings.dedup();
    graph.metadata_warnings.sort();
    graph.metadata_warnings.dedup();
    graph.migration_warnings.sort();
    graph.migration_warnings.dedup();
    graph.cycle_errors = detect_cycles(&graph.edges);
    graph.node_cache_key_parts = graph
        .nodes
        .iter()
        .map(|node| node.cache_key_parts.clone())
        .collect();
    graph.cache_key_parts = cache_key_parts_for_ref(&graph.root_ref);
    if let Some(root) = graph
        .nodes
        .iter()
        .find(|node| node.reference == graph.root_ref)
    {
        graph.cache_key_parts.content_hash = root.content_hash.clone();
    }
    graph.stable_cache_key = stable_graph_cache_key(graph);
    graph.debug_log.push(format!("assets.graph.resolve_v1: finalized schema='{}' nodes={} edges={} missing={} cycles={} cache_key='{}'", graph.schema, graph.nodes.len(), graph.edges.len(), graph.missing_refs.len(), graph.cycle_errors.len(), graph.stable_cache_key));
}

pub(super) fn push_node(
    graph: &mut ResolvedAssetGraphV2,
    reference: &str,
    role: &str,
    asset_kind: &str,
    semantic_gateway: &str,
    method: &str,
) {
    let reference = normalize_asset_ref(reference);
    if reference.is_empty() {
        return;
    }
    graph.nodes.push(AssetGraphNode {
        id: stable_graph_id(&reference),
        reference: reference.clone(),
        asset_ref: reference.clone(),
        role: role.to_owned(),
        kind: asset_kind.to_owned(),
        asset_kind: asset_kind.to_owned(),
        byte_owner: "engine.assets".to_owned(),
        semantic_gateway: semantic_gateway.to_owned(),
        method: method.to_owned(),
        semantic_owner: asset_kind.to_owned(),
        cache_key_parts: cache_key_parts_for_ref(&reference),
        vfs_source: AssetGraphVfsSource {
            logical_path: split_ref(&reference).0,
            ..AssetGraphVfsSource::default()
        },
        metadata_namespaces: Vec::new(),
        schema_version: "v2".to_owned(),
        ..Default::default()
    });
}

pub(super) fn push_edge(
    graph: &mut ResolvedAssetGraphV2,
    from: &str,
    to: &str,
    kind: &str,
    required: bool,
) {
    let from_ref = normalize_asset_ref(from);
    let to_ref = normalize_asset_ref(to);
    if from_ref.trim().is_empty() || to_ref.trim().is_empty() {
        return;
    }
    graph.edges.push(AssetGraphEdge {
        from: stable_graph_id(&from_ref),
        to: stable_graph_id(&to_ref),
        from_ref,
        to_ref,
        kind: kind.to_owned(),
        required,
    });
}

pub fn classify_asset_ref(
    reference: &str,
) -> (&'static str, &'static str, &'static str, &'static str) {
    classify_ref(reference)
}

fn detect_cycles(edges: &[AssetGraphEdge]) -> Vec<String> {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for edge in edges {
        adjacency
            .entry(edge.from_ref.clone())
            .or_default()
            .push(edge.to_ref.clone());
    }
    let mut errors = Vec::new();
    let mut visiting = BTreeSet::<String>::new();
    let mut visited = BTreeSet::<String>::new();
    for node in adjacency.keys() {
        let mut stack = Vec::<String>::new();
        visit_cycle(
            node,
            &adjacency,
            &mut visiting,
            &mut visited,
            &mut stack,
            &mut errors,
        );
    }
    errors.sort();
    errors.dedup();
    errors
}

fn visit_cycle(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if visited.contains(node) {
        return;
    }
    if !visiting.insert(node.to_owned()) {
        let mut cycle = stack.clone();
        cycle.push(node.to_owned());
        errors.push(cycle.join(" -> "));
        return;
    }
    stack.push(node.to_owned());
    if let Some(next) = adjacency.get(node) {
        for child in next {
            visit_cycle(child, adjacency, visiting, visited, stack, errors);
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node.to_owned());
}
