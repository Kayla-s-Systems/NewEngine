use serde::{Deserialize, Serialize};

use crate::{DataDrivenConstructionPlan, ROLE_DRAWABLE_DICTIONARY, ROLE_TEXTURE_DICTIONARY};

pub const ASSET_GRAPH_SCHEMA: &str = "newengine.asset_graph.resolved.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphResolveRequest {
    pub source: String,
}
impl Default for AssetGraphResolveRequest { fn default() -> Self { Self { source: String::new() } } }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphNode {
    pub id: String,
    pub reference: String,
    pub role: String,
    pub asset_kind: String,
}
impl Default for AssetGraphNode { fn default() -> Self { Self { id: String::new(), reference: String::new(), role: String::new(), asset_kind: String::new() } } }

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
pub struct ResolvedAssetGraph {
    pub schema: String,
    pub source: String,
    pub nodes: Vec<AssetGraphNode>,
    pub edges: Vec<AssetGraphEdge>,
    pub missing_refs: Vec<String>,
    pub cycle_errors: Vec<String>,
    pub format_warnings: Vec<String>,
    pub migration_warnings: Vec<String>,
    pub cache_key_policy: String,
}
impl Default for ResolvedAssetGraph {
    fn default() -> Self {
        Self {
            schema: ASSET_GRAPH_SCHEMA.to_owned(),
            source: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            missing_refs: Vec::new(),
            cycle_errors: Vec::new(),
            format_warnings: Vec::new(),
            migration_warnings: Vec::new(),
            cache_key_policy: "logical_path + entry + content_hash + import_settings_hash".to_owned(),
        }
    }
}

pub struct AssetGraphResolver;

impl AssetGraphResolver {
    pub fn resolve_construction_plan(plan: &DataDrivenConstructionPlan) -> ResolvedAssetGraph {
        let mut graph = ResolvedAssetGraph { source: plan.source.clone(), ..Default::default() };
        for object in &plan.objects {
            let definition_ref = object.definition.logical_path.clone();
            push_node(&mut graph, &definition_ref, "definition_entries", &object.definition.asset_kind);
            if let Some(drawable) = object.drawable.as_ref() {
                push_node(&mut graph, &drawable.logical_path, ROLE_DRAWABLE_DICTIONARY, &drawable.asset_kind);
                push_edge(&mut graph, &definition_ref, &drawable.logical_path, ROLE_DRAWABLE_DICTIONARY, drawable.required);
            } else {
                graph.missing_refs.push(format!("{}: missing drawable dictionary", object.name));
            }
            if let Some(texture_dictionary) = object.texture_dictionary.as_ref() {
                push_node(&mut graph, &texture_dictionary.logical_path, ROLE_TEXTURE_DICTIONARY, &texture_dictionary.asset_kind);
                push_edge(&mut graph, &definition_ref, &texture_dictionary.logical_path, ROLE_TEXTURE_DICTIONARY, texture_dictionary.required);
            } else {
                graph.missing_refs.push(format!("{}: missing texture dictionary", object.name));
            }
            if let Some(physics) = object.physics_dictionary.as_ref() {
                push_node(&mut graph, &physics.logical_path, "physics_dictionary", &physics.asset_kind);
                push_edge(&mut graph, &definition_ref, &physics.logical_path, "physics_dictionary", physics.required);
            }
            for slot in &object.material_slots {
                if slot.material.trim().is_empty() {
                    graph.missing_refs.push(format!("{}: material slot '{}' has empty material ref", object.name, slot.slot));
                    continue;
                }
                push_node(&mut graph, &slot.material, "material", "material_library");
                if let Some(drawable) = object.drawable.as_ref() {
                    push_edge(&mut graph, &drawable.logical_path, &slot.material, &format!("material_slot:{}", slot.slot), true);
                } else {
                    push_edge(&mut graph, &definition_ref, &slot.material, &format!("material_slot:{}", slot.slot), true);
                }
            }
        }
        for warning in &plan.warnings {
            if warning.to_ascii_lowercase().contains("neytd") {
                graph.migration_warnings.push(warning.clone());
            } else {
                graph.format_warnings.push(warning.clone());
            }
        }
        graph.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        graph.nodes.dedup_by(|a, b| a.id == b.id);
        graph.edges.sort_by(|a, b| (a.from.as_str(), a.to.as_str(), a.kind.as_str()).cmp(&(b.from.as_str(), b.to.as_str(), b.kind.as_str())));
        graph.edges.dedup_by(|a, b| a.from == b.from && a.to == b.to && a.kind == b.kind);
        graph.missing_refs.sort();
        graph.missing_refs.dedup();
        graph
    }
}

fn push_node(graph: &mut ResolvedAssetGraph, reference: &str, role: &str, asset_kind: &str) {
    let reference = reference.trim();
    if reference.is_empty() { return; }
    graph.nodes.push(AssetGraphNode { id: stable_graph_id(reference), reference: reference.to_owned(), role: role.to_owned(), asset_kind: asset_kind.to_owned() });
}

fn push_edge(graph: &mut ResolvedAssetGraph, from: &str, to: &str, kind: &str, required: bool) {
    if from.trim().is_empty() || to.trim().is_empty() { return; }
    graph.edges.push(AssetGraphEdge { from: stable_graph_id(from), to: stable_graph_id(to), kind: kind.to_owned(), required });
}

fn stable_graph_id(reference: &str) -> String {
    let normalized = reference.trim().replace('\\', "/");
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
