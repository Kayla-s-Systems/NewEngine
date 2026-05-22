use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    DataDrivenConstructionPlan, DRAWABLE_DICTIONARY_ASSET_KIND, MATERIAL_LIBRARY_ASSET_KIND,
    OBJECT_TYPE_DEFINITIONS_ASSET_KIND, ROLE_DEFINITION_ENTRIES, ROLE_DRAWABLE_DICTIONARY,
    ROLE_MATERIAL_LIBRARY, ROLE_TEXTURE_DICTIONARY, TEXTURE_DICTIONARY_ASSET_KIND,
};

pub const ASSET_GRAPH_SCHEMA: &str = "newengine.asset_graph.resolved.v1";
pub const ASSET_GRAPH_RESOLVED_SCHEMA_V1: &str = ASSET_GRAPH_SCHEMA;
pub const ENGINE_ASSET_GRAPH_SERVICE_ID: &str = "engine.asset_graph";
pub const ASSET_GRAPH_SERVICE_ID: &str = "asset_graph.api";
pub const ASSET_GRAPH_BACKEND_CAPABILITY_ID: &str = "asset_graph.backend";
pub const ASSET_GRAPH_METHOD_RESOLVE_V1: &str = "asset_graph.resolve_v1";
pub const ASSET_GRAPH_METHOD_VALIDATE_V1: &str = "asset_graph.validate_v1";
pub const ASSET_GRAPH_METHOD_DUMP_JSON_V1: &str = "asset_graph.dump_json_v1";
pub const ASSET_GRAPH_METHODS: &[&str] = &[
    ASSET_GRAPH_METHOD_RESOLVE_V1,
    ASSET_GRAPH_METHOD_VALIDATE_V1,
    ASSET_GRAPH_METHOD_DUMP_JSON_V1,
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphResolveRequest {
    pub root_ref: String,
    /// Backward-compatible request field used by older `engine.model` callers.
    /// New callers should use `root_ref`.
    pub source: String,
}
impl Default for AssetGraphResolveRequest { fn default() -> Self { Self { root_ref: String::new(), source: String::new() } } }

impl AssetGraphResolveRequest {
    #[inline]
    pub fn root(&self) -> &str {
        let root = self.root_ref.trim();
        if root.is_empty() { self.source.trim() } else { root }
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
            schema_version: "v1".to_owned(),
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
}
impl Default for AssetGraphVfsSource {
    fn default() -> Self {
        Self { source_kind: "unresolved".to_owned(), logical_path: String::new(), physical_path: None, package_path: None, package_entry: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphNode {
    pub id: String,
    pub reference: String,
    pub role: String,
    pub asset_kind: String,
    pub semantic_gateway: String,
    pub vfs_source: AssetGraphVfsSource,
    pub cache_key_parts: AssetGraphCacheKeyParts,
    pub metadata_namespaces: Vec<String>,
}
impl Default for AssetGraphNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            reference: String::new(),
            role: String::new(),
            asset_kind: String::new(),
            semantic_gateway: String::new(),
            vfs_source: AssetGraphVfsSource::default(),
            cache_key_parts: AssetGraphCacheKeyParts::default(),
            metadata_namespaces: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub required: bool,
}
impl Default for AssetGraphEdge { fn default() -> Self { Self { from: String::new(), to: String::new(), kind: String::new(), required: true } } }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedAssetGraphV1 {
    pub schema: String,
    pub root_ref: String,
    /// Compatibility projection for older model-domain callers.
    pub source: String,
    pub nodes: Vec<AssetGraphNode>,
    pub edges: Vec<AssetGraphEdge>,
    pub missing_refs: Vec<String>,
    pub cycle_errors: Vec<String>,
    pub format_warnings: Vec<String>,
    pub metadata_warnings: Vec<String>,
    pub migration_warnings: Vec<String>,
    pub stable_cache_key: String,
    pub cache_key_policy: String,
    pub cache_key_parts: AssetGraphCacheKeyParts,
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
            stable_cache_key: String::new(),
            cache_key_policy: "logical_path + entry + content_hash + schema_version + import_settings_hash + provider_version".to_owned(),
            cache_key_parts: AssetGraphCacheKeyParts::default(),
            debug_log: Vec::new(),
        }
    }
}

pub type ResolvedAssetGraph = ResolvedAssetGraphV1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphValidationResult {
    pub valid: bool,
    pub root_ref: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub graph: Option<ResolvedAssetGraphV1>,
}
impl Default for AssetGraphValidationResult {
    fn default() -> Self { Self { valid: false, root_ref: String::new(), errors: Vec::new(), warnings: Vec::new(), graph: None } }
}

pub struct AssetGraphResolver;

impl AssetGraphResolver {
    pub fn resolve_root_ref(root_ref: &str) -> ResolvedAssetGraphV1 {
        let root_ref = normalize_asset_ref(root_ref);
        let mut graph = ResolvedAssetGraphV1 {
            root_ref: root_ref.clone(),
            source: root_ref.clone(),
            cache_key_parts: cache_key_parts_for_ref(&root_ref),
            ..Default::default()
        };
        graph.debug_log.push(format!("asset_graph.resolve_v1: begin root_ref='{root_ref}'"));
        let (role, kind, gateway) = classify_ref(&root_ref);
        push_node(&mut graph, &root_ref, role, kind, gateway);
        graph.stable_cache_key = stable_cache_key(&graph.cache_key_parts);
        graph.debug_log.push(format!("asset_graph.resolve_v1: root classified role='{role}' semantic_gateway='{gateway}'"));
        graph
    }

    pub fn resolve_construction_plan(plan: &DataDrivenConstructionPlan) -> ResolvedAssetGraphV1 {
        let root = plan.source.trim();
        let mut graph = ResolvedAssetGraphV1 {
            root_ref: root.to_owned(),
            source: root.to_owned(),
            cache_key_parts: cache_key_parts_for_ref(root),
            ..Default::default()
        };
        graph.debug_log.push(format!("asset_graph.resolve_v1: begin construction_plan source='{root}' objects={}", plan.objects.len()));
        for object in &plan.objects {
            let definition_ref = normalize_asset_ref(&object.definition.logical_path);
            push_node(&mut graph, &definition_ref, ROLE_DEFINITION_ENTRIES, &object.definition.asset_kind, "engine.definitions");
            if let Some(drawable) = object.drawable.as_ref() {
                let drawable_ref = normalize_asset_ref(&drawable.logical_path);
                push_node(&mut graph, &drawable_ref, ROLE_DRAWABLE_DICTIONARY, &drawable.asset_kind, "engine.model");
                push_edge(&mut graph, &definition_ref, &drawable_ref, ROLE_DRAWABLE_DICTIONARY, drawable.required);
            } else {
                graph.missing_refs.push(format!("{}: missing drawable dictionary", object.name));
            }
            if let Some(texture_dictionary) = object.texture_dictionary.as_ref() {
                let texture_ref = normalize_asset_ref(&texture_dictionary.logical_path);
                push_node(&mut graph, &texture_ref, ROLE_TEXTURE_DICTIONARY, &texture_dictionary.asset_kind, "engine.textures");
                push_edge(&mut graph, &definition_ref, &texture_ref, ROLE_TEXTURE_DICTIONARY, texture_dictionary.required);
            } else {
                graph.missing_refs.push(format!("{}: missing texture dictionary", object.name));
            }
            if let Some(physics) = object.physics_dictionary.as_ref() {
                let physics_ref = normalize_asset_ref(&physics.logical_path);
                push_node(&mut graph, &physics_ref, "physics_dictionary", &physics.asset_kind, "engine.physics");
                push_edge(&mut graph, &definition_ref, &physics_ref, "physics_dictionary", physics.required);
            }
            for slot in &object.material_slots {
                if slot.material.trim().is_empty() {
                    graph.missing_refs.push(format!("{}: material slot '{}' has empty material ref", object.name, slot.slot));
                    continue;
                }
                let material_ref = normalize_asset_ref(&slot.material);
                push_node(&mut graph, &material_ref, ROLE_MATERIAL_LIBRARY, MATERIAL_LIBRARY_ASSET_KIND, "engine.materials");
                if let Some(drawable) = object.drawable.as_ref() {
                    push_edge(&mut graph, &drawable.logical_path, &material_ref, &format!("material_slot/{}", slot.slot), true);
                } else {
                    push_edge(&mut graph, &definition_ref, &material_ref, &format!("material_slot/{}", slot.slot), true);
                }
            }
            graph.debug_log.push(format!("asset_graph.resolve_v1: object='{}' graph nodes={} edges={}", object.name, graph.nodes.len(), graph.edges.len()));
        }
        for warning in &plan.warnings {
            if warning.to_ascii_lowercase().contains("neytd") {
                graph.migration_warnings.push(warning.clone());
            } else {
                graph.format_warnings.push(warning.clone());
            }
        }
        finalize_graph(&mut graph);
        graph
    }

    pub fn validate_graph(graph: ResolvedAssetGraphV1) -> AssetGraphValidationResult {
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
        errors.extend(graph.missing_refs.iter().map(|it| format!("missing ref: {it}")));
        errors.extend(graph.cycle_errors.iter().map(|it| format!("cycle: {it}")));
        AssetGraphValidationResult { valid: errors.is_empty(), root_ref: graph.root_ref.clone(), errors, warnings, graph: Some(graph) }
    }
}

pub fn push_manifest_dependency(
    graph: &mut ResolvedAssetGraphV1,
    owner_ref: &str,
    reference: &str,
    role: &str,
    required: bool,
) {
    let reference = normalize_asset_ref(reference);
    if reference.trim().is_empty() {
        graph.metadata_warnings.push(format!("empty dependency reference from '{owner_ref}' role='{role}'"));
        return;
    }
    let (node_role, asset_kind, gateway) = classify_ref(&reference);
    push_node(graph, &reference, node_role, asset_kind, gateway);
    push_edge(graph, owner_ref, &reference, role, required);
}

pub fn attach_vfs_source(graph: &mut ResolvedAssetGraphV1, reference: &str, source: AssetGraphVfsSource) {
    let id = stable_graph_id(reference);
    for node in &mut graph.nodes {
        if node.id == id {
            node.vfs_source = source.clone();
        }
    }
}

pub fn attach_content_hash(graph: &mut ResolvedAssetGraphV1, reference: &str, content_hash: impl Into<String>) {
    let id = stable_graph_id(reference);
    let content_hash = content_hash.into();
    for node in &mut graph.nodes {
        if node.id == id {
            node.cache_key_parts.content_hash = Some(content_hash.clone());
        }
    }
}

pub fn finalize_graph(graph: &mut ResolvedAssetGraphV1) {
    graph.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    graph.nodes.dedup_by(|a, b| a.id == b.id);
    graph.edges.sort_by(|a, b| (a.from.as_str(), a.to.as_str(), a.kind.as_str()).cmp(&(b.from.as_str(), b.to.as_str(), b.kind.as_str())));
    graph.edges.dedup_by(|a, b| a.from == b.from && a.to == b.to && a.kind == b.kind);
    graph.missing_refs.sort();
    graph.missing_refs.dedup();
    graph.cycle_errors = detect_cycles(&graph.edges);
    graph.stable_cache_key = stable_cache_key(&graph.cache_key_parts);
    graph.debug_log.push(format!("asset_graph.resolve_v1: finalized nodes={} edges={} missing={} cycles={}", graph.nodes.len(), graph.edges.len(), graph.missing_refs.len(), graph.cycle_errors.len()));
}

fn push_node(graph: &mut ResolvedAssetGraphV1, reference: &str, role: &str, asset_kind: &str, semantic_gateway: &str) {
    let reference = normalize_asset_ref(reference);
    if reference.is_empty() { return; }
    graph.nodes.push(AssetGraphNode {
        id: stable_graph_id(&reference),
        reference: reference.clone(),
        role: role.to_owned(),
        asset_kind: asset_kind.to_owned(),
        semantic_gateway: semantic_gateway.to_owned(),
        cache_key_parts: cache_key_parts_for_ref(&reference),
        vfs_source: AssetGraphVfsSource { logical_path: split_ref(&reference).0, ..AssetGraphVfsSource::default() },
        metadata_namespaces: Vec::new(),
    });
}

fn push_edge(graph: &mut ResolvedAssetGraphV1, from: &str, to: &str, kind: &str, required: bool) {
    let from = normalize_asset_ref(from);
    let to = normalize_asset_ref(to);
    if from.trim().is_empty() || to.trim().is_empty() { return; }
    graph.edges.push(AssetGraphEdge { from: stable_graph_id(&from), to: stable_graph_id(&to), kind: kind.to_owned(), required });
}

fn classify_ref(reference: &str) -> (&'static str, &'static str, &'static str) {
    let (path, _) = split_ref(reference);
    let ext = path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "ytyp" => (ROLE_DEFINITION_ENTRIES, OBJECT_TYPE_DEFINITIONS_ASSET_KIND, "engine.definitions"),
        "ydd" => (ROLE_DRAWABLE_DICTIONARY, DRAWABLE_DICTIONARY_ASSET_KIND, "engine.model"),
        "nemat" => (ROLE_MATERIAL_LIBRARY, MATERIAL_LIBRARY_ASSET_KIND, "engine.materials"),
        "ytd" => (ROLE_TEXTURE_DICTIONARY, TEXTURE_DICTIONARY_ASSET_KIND, "engine.textures"),
        "ybn" | "ycol" => ("physics_dictionary", "physics_dictionary", "engine.physics"),
        _ => ("asset", "unknown", "engine.assets"),
    }
}

fn detect_cycles(edges: &[AssetGraphEdge]) -> Vec<String> {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for edge in edges {
        adjacency.entry(edge.from.clone()).or_default().push(edge.to.clone());
    }
    let mut errors = Vec::new();
    let mut visiting = BTreeSet::<String>::new();
    let mut visited = BTreeSet::<String>::new();
    for node in adjacency.keys() {
        let mut stack = Vec::<String>::new();
        visit_cycle(node, &adjacency, &mut visiting, &mut visited, &mut stack, &mut errors);
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
    if visited.contains(node) { return; }
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

fn cache_key_parts_for_ref(reference: &str) -> AssetGraphCacheKeyParts {
    let (logical_path, entry) = split_ref(reference);
    AssetGraphCacheKeyParts { logical_path, entry, schema_version: "v1".to_owned(), ..Default::default() }
}

fn stable_cache_key(parts: &AssetGraphCacheKeyParts) -> String {
    let key = format!(
        "{}@{}|{}|{}|{}|{}",
        parts.logical_path,
        parts.entry.clone().unwrap_or_default(),
        parts.content_hash.clone().unwrap_or_default(),
        parts.schema_version,
        parts.import_settings_hash.clone().unwrap_or_default(),
        parts.provider_version.clone().unwrap_or_default(),
    );
    format!("asset-graph:{:016x}", fnv1a64(key.as_bytes()))
}

fn split_ref(reference: &str) -> (String, Option<String>) {
    let normalized = normalize_asset_ref(reference);
    match normalized.rsplit_once('@') {
        Some((path, entry)) if !entry.trim().is_empty() => (path.to_owned(), Some(entry.trim().to_owned())),
        _ => (normalized, None),
    }
}

fn normalize_asset_ref(reference: &str) -> String {
    let mut s = reference.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") { s = rest.to_owned(); }
    while s.contains("//") { s = s.replace("//", "/"); }
    s.trim_start_matches('/').to_owned()
}

fn stable_graph_id(reference: &str) -> String {
    let normalized = normalize_asset_ref(reference);
    format!("asset:{:016x}", fnv1a64(normalized.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataDrivenAssetLink, DataDrivenObjectConstruction};

    #[test]
    fn root_ref_classifies_ytyp_as_definitions_not_scene() {
        let graph = AssetGraphResolver::resolve_root_ref("world/foo.ytyp@bar");
        let root = graph.nodes.iter().find(|node| node.reference == "world/foo.ytyp@bar").unwrap();
        assert_eq!(root.semantic_gateway, "engine.definitions");
        assert_ne!(root.semantic_gateway, "engine.scene");
        assert_eq!(root.role, ROLE_DEFINITION_ENTRIES);
    }

    #[test]
    fn construction_plan_builds_declarative_edges() {
        let plan = DataDrivenConstructionPlan {
            source: "world/foo.ytyp".to_owned(),
            objects: vec![DataDrivenObjectConstruction {
                name: "bar".to_owned(),
                definition: DataDrivenAssetLink { logical_path: "world/foo.ytyp@bar".to_owned(), asset_kind: OBJECT_TYPE_DEFINITIONS_ASSET_KIND.to_owned(), extension: "ytyp".to_owned(), required: true, ..Default::default() },
                drawable: Some(DataDrivenAssetLink { logical_path: "models/foo.ydd@bar".to_owned(), asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(), extension: "ydd".to_owned(), required: true, ..Default::default() }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let graph = AssetGraphResolver::resolve_construction_plan(&plan);
        assert!(graph.edges.iter().any(|edge| edge.kind == ROLE_DRAWABLE_DICTIONARY));
        assert!(graph.debug_log.iter().any(|line| line.contains("asset_graph.resolve_v1")));
    }
}
