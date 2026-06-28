use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    DataDrivenConstructionPlan, MATERIAL_LIBRARY_ASSET_KIND, ROLE_DEFINITION_ENTRIES,
    ROLE_DRAWABLE_DICTIONARY, ROLE_MATERIAL_LIBRARY, ROLE_TEXTURE_DICTIONARY,
};

mod routing;
use routing::classify_ref;

pub const ASSET_GRAPH_SCHEMA: &str = "newengine.assets.graph.resolved.v2";
pub const ASSET_GRAPH_RESOLVED_SCHEMA_V1: &str = ASSET_GRAPH_SCHEMA;
pub const ASSET_GRAPH_RESOLVED_SCHEMA_V2: &str = ASSET_GRAPH_SCHEMA;
pub const ENGINE_ASSETS_GRAPH_SERVICE_ID: &str = "engine.assets.graph";
pub const ASSET_GRAPH_SERVICE_ID: &str = "asset_graph.api";
pub const ASSET_GRAPH_BACKEND_CAPABILITY_ID: &str = "assets.graph.backend";
pub const ASSET_GRAPH_METHOD_RESOLVE_V1: &str = "assets.graph.resolve_v1";
pub const ASSET_GRAPH_METHOD_VALIDATE_V1: &str = "assets.graph.validate_v1";
pub const ASSET_GRAPH_METHOD_DUMP_JSON_V1: &str = "assets.graph.dump_json_v1";
pub const ASSET_GRAPH_METHODS: &[&str] = &[
    newengine_service_api::SERVICE_METHOD_INFO_JSON,
    newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
    ASSET_GRAPH_METHOD_RESOLVE_V1,
    ASSET_GRAPH_METHOD_VALIDATE_V1,
    ASSET_GRAPH_METHOD_DUMP_JSON_V1,
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphResolveRequest {
    pub root_ref: String,
}
impl Default for AssetGraphResolveRequest {
    fn default() -> Self {
        Self {
            root_ref: String::new(),
        }
    }
}

impl AssetGraphResolveRequest {
    #[inline]
    pub fn root(&self) -> &str {
        self.root_ref.trim()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphCacheKeyParts {
    pub logical_path: String,
    pub entry: Option<String>,
    pub content_hash: Option<String>,
    pub schema_version: String,
    pub import_settings_hash: Option<String>,
    pub provider_version: Option<String>,
}
impl Default for AssetGraphCacheKeyParts {
    fn default() -> Self {
        Self {
            logical_path: String::new(),
            entry: None,
            content_hash: None,
            schema_version: "v2".to_owned(),
            import_settings_hash: None,
            provider_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphVfsSource {
    pub source_kind: String,
    pub logical_path: String,
    pub physical_path: Option<String>,
    pub package_path: Option<String>,
    pub package_entry: Option<String>,
    pub layer_id: Option<String>,
    pub overridden_by: Vec<String>,
}
impl Default for AssetGraphVfsSource {
    fn default() -> Self {
        Self {
            source_kind: "unresolved".to_owned(),
            logical_path: String::new(),
            physical_path: None,
            package_path: None,
            package_entry: None,
            layer_id: None,
            overridden_by: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphNode {
    pub id: String,
    pub reference: String,
    #[serde(rename = "ref")]
    pub asset_ref: String,
    pub role: String,
    pub kind: String,
    pub asset_kind: String,
    pub byte_owner: String,
    pub semantic_gateway: String,
    pub method: String,
    pub semantic_owner: String,
    pub vfs_source: AssetGraphVfsSource,
    pub content_hash: Option<String>,
    pub entry_hash: Option<String>,
    pub schema_version: String,
    pub cache_key_parts: AssetGraphCacheKeyParts,
    pub metadata_namespaces: Vec<String>,
    pub warnings: Vec<String>,
}
impl Default for AssetGraphNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            reference: String::new(),
            asset_ref: String::new(),
            role: String::new(),
            kind: String::new(),
            asset_kind: String::new(),
            byte_owner: "engine.assets".to_owned(),
            semantic_gateway: String::new(),
            method: String::new(),
            semantic_owner: String::new(),
            vfs_source: AssetGraphVfsSource::default(),
            content_hash: None,
            entry_hash: None,
            schema_version: "v2".to_owned(),
            cache_key_parts: AssetGraphCacheKeyParts::default(),
            metadata_namespaces: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphEdge {
    pub from: String,
    pub to: String,
    pub from_ref: String,
    pub to_ref: String,
    pub kind: String,
    pub required: bool,
}
impl Default for AssetGraphEdge {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            from_ref: String::new(),
            to_ref: String::new(),
            kind: String::new(),
            required: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedAssetGraphV1 {
    pub schema: String,
    pub root_ref: String,
    pub source: String,
    pub nodes: Vec<AssetGraphNode>,
    pub edges: Vec<AssetGraphEdge>,
    pub missing_refs: Vec<String>,
    pub cycle_errors: Vec<String>,
    pub format_warnings: Vec<String>,
    pub metadata_warnings: Vec<String>,
    pub migration_warnings: Vec<String>,
    pub cache_key_parts: AssetGraphCacheKeyParts,
    pub node_cache_key_parts: Vec<AssetGraphCacheKeyParts>,
    pub stable_cache_key: String,
    pub cache_key_policy: String,
    pub debug_log: Vec<String>,
}
impl Default for ResolvedAssetGraphV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_GRAPH_SCHEMA.to_owned(),
            root_ref: String::new(),
            source: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            missing_refs: Vec::new(),
            cycle_errors: Vec::new(),
            format_warnings: Vec::new(),
            metadata_warnings: Vec::new(),
            migration_warnings: Vec::new(),
            cache_key_parts: AssetGraphCacheKeyParts::default(),
            node_cache_key_parts: Vec::new(),
            stable_cache_key: String::new(),
            cache_key_policy: "graph(root_ref + ordered nodes + ordered edges + content_hash + entry_hash + schema_version + provider_version)".to_owned(),
            debug_log: Vec::new(),
        }
    }
}

pub type ResolvedAssetGraphV2 = ResolvedAssetGraphV1;
pub type ResolvedAssetGraph = ResolvedAssetGraphV2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphValidationResult {
    pub valid: bool,
    pub root_ref: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub graph: Option<ResolvedAssetGraphV2>,
}
impl Default for AssetGraphValidationResult {
    fn default() -> Self {
        Self {
            valid: false,
            root_ref: String::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            graph: None,
        }
    }
}

pub struct AssetGraphResolver;

impl AssetGraphResolver {
    /// Classification-only resolver kept for dry-run tests and callers that cannot
    /// access `engine.assets`. Runtime `engine.assets.graph` hydrates this graph by
    /// calling semantic gateways and attaching VFS/hash diagnostics.
    pub fn resolve_root_ref(root_ref: &str) -> ResolvedAssetGraphV2 {
        let root_ref = normalize_asset_ref(root_ref);
        let mut graph = ResolvedAssetGraphV2 {
            root_ref: root_ref.clone(),
            source: root_ref.clone(),
            cache_key_parts: cache_key_parts_for_ref(&root_ref),
            ..Default::default()
        };
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: begin root_ref='{root_ref}' mode='classification-only'"
        ));
        let (role, kind, gateway, method) = classify_ref(&root_ref);
        push_node(&mut graph, &root_ref, role, kind, gateway, method);
        finalize_graph(&mut graph);
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: root classified role='{role}' semantic_gateway='{gateway}'"
        ));
        graph
    }

    pub fn resolve_construction_plan(plan: &DataDrivenConstructionPlan) -> ResolvedAssetGraphV2 {
        let root = plan.source.trim();
        let mut graph = ResolvedAssetGraphV2 {
            root_ref: root.to_owned(),
            source: root.to_owned(),
            cache_key_parts: cache_key_parts_for_ref(root),
            ..Default::default()
        };
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: begin construction_plan source='{root}' objects={}",
            plan.objects.len()
        ));
        for object in &plan.objects {
            let definition_ref = normalize_asset_ref(&object.definition.logical_path);
            push_node(
                &mut graph,
                &definition_ref,
                ROLE_DEFINITION_ENTRIES,
                &object.definition.asset_kind,
                "engine.definitions",
                "definitions.entry_json_v1",
            );
            if let Some(drawable) = object.drawable.as_ref() {
                let drawable_ref = normalize_asset_ref(&drawable.logical_path);
                push_node(
                    &mut graph,
                    &drawable_ref,
                    ROLE_DRAWABLE_DICTIONARY,
                    &drawable.asset_kind,
                    "engine.model",
                    "model.drawable_dictionary_manifest_json_v1",
                );
                push_edge(
                    &mut graph,
                    &definition_ref,
                    &drawable_ref,
                    ROLE_DRAWABLE_DICTIONARY,
                    drawable.required,
                );
            } else {
                graph
                    .missing_refs
                    .push(format!("{}: missing drawable dictionary", object.name));
            }
            if let Some(texture_dictionary) = object.texture_dictionary.as_ref() {
                let texture_ref = normalize_asset_ref(&texture_dictionary.logical_path);
                push_node(
                    &mut graph,
                    &texture_ref,
                    ROLE_TEXTURE_DICTIONARY,
                    &texture_dictionary.asset_kind,
                    "engine.assets",
                    "asset.texture_dictionary_runtime_v1",
                );
                push_edge(
                    &mut graph,
                    &definition_ref,
                    &texture_ref,
                    ROLE_TEXTURE_DICTIONARY,
                    texture_dictionary.required,
                );
            } else {
                graph
                    .missing_refs
                    .push(format!("{}: missing texture dictionary", object.name));
            }
            if let Some(physics) = object.physics_dictionary.as_ref() {
                let physics_ref = normalize_asset_ref(&physics.logical_path);
                push_node(
                    &mut graph,
                    &physics_ref,
                    "physics_dictionary",
                    &physics.asset_kind,
                    "engine.physics",
                    "physics.frame_json_v1",
                );
                push_edge(
                    &mut graph,
                    &definition_ref,
                    &physics_ref,
                    "physics_dictionary",
                    physics.required,
                );
            }
            for slot in &object.material_slots {
                if slot.material.trim().is_empty() {
                    graph.missing_refs.push(format!(
                        "{}: material slot '{}' has empty material ref",
                        object.name, slot.slot
                    ));
                    continue;
                }
                let material_ref = normalize_asset_ref(&slot.material);
                push_node(
                    &mut graph,
                    &material_ref,
                    ROLE_MATERIAL_LIBRARY,
                    MATERIAL_LIBRARY_ASSET_KIND,
                    "engine.materials",
                    "materials.load_descriptor_v1",
                );
                if let Some(drawable) = object.drawable.as_ref() {
                    push_edge(
                        &mut graph,
                        &drawable.logical_path,
                        &material_ref,
                        &format!("material_slot/{}", slot.slot),
                        true,
                    );
                } else {
                    push_edge(
                        &mut graph,
                        &definition_ref,
                        &material_ref,
                        &format!("material_slot/{}", slot.slot),
                        true,
                    );
                }
            }
            graph.debug_log.push(format!(
                "assets.graph.resolve_v1: object='{}' graph nodes={} edges={}",
                object.name,
                graph.nodes.len(),
                graph.edges.len()
            ));
        }
        for warning in &plan.warnings {
            if warning
                .to_ascii_lowercase()
                .contains("retired texture dictionary")
            {
                graph.migration_warnings.push(warning.clone());
            } else {
                graph.format_warnings.push(warning.clone());
            }
        }
        finalize_graph(&mut graph);
        graph
    }

    pub fn validate_graph(graph: ResolvedAssetGraphV2) -> AssetGraphValidationResult {
        let mut errors = Vec::new();
        let mut warnings = graph.format_warnings.clone();
        warnings.extend(graph.metadata_warnings.clone());
        warnings.extend(graph.migration_warnings.clone());
        if graph.root_ref.trim().is_empty() {
            errors.push("asset graph root_ref is empty".to_owned());
        }
        if graph.nodes.is_empty() {
            errors.push("asset graph contains no nodes".to_owned());
        }
        errors.extend(
            graph
                .missing_refs
                .iter()
                .map(|it| format!("missing ref: {it}")),
        );
        errors.extend(graph.cycle_errors.iter().map(|it| format!("cycle: {it}")));
        AssetGraphValidationResult {
            valid: errors.is_empty(),
            root_ref: graph.root_ref.clone(),
            errors,
            warnings,
            graph: Some(graph),
        }
    }
}

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

fn push_node(
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

fn push_edge(graph: &mut ResolvedAssetGraphV2, from: &str, to: &str, kind: &str, required: bool) {
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

pub fn cache_key_parts_for_ref(reference: &str) -> AssetGraphCacheKeyParts {
    let (logical_path, entry) = split_ref(reference);
    AssetGraphCacheKeyParts {
        logical_path,
        entry,
        schema_version: "v2".to_owned(),
        ..Default::default()
    }
}

fn stable_graph_cache_key(graph: &ResolvedAssetGraphV2) -> String {
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

fn split_ref(reference: &str) -> (String, Option<String>) {
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
