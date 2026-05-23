use serde::{Deserialize, Serialize};

use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_model_domain_api::ResolvedAssetGraphV1;
use newengine_scene::spawn_named;
use newengine_transform::Transform;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionInstantiateTransform {
    pub translation: [f32; 3],
    pub rotation_ypr: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for DefinitionInstantiateTransform {
    fn default() -> Self {
        Self { translation: [0.0, 0.0, 0.0], rotation_ypr: [0.0, 0.0, 0.0], scale: [1.0, 1.0, 1.0] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeCommand {
    InstantiateDefinition {
        definition_ref: String,
        transform: DefinitionInstantiateTransform,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityCommandTrace {
    pub command: String,
    pub entity_name: String,
    pub transform: DefinitionInstantiateTransform,
}

impl Default for EntityCommandTrace {
    fn default() -> Self { Self { command: "EntityCommand::Spawn".to_owned(), entity_name: String::new(), transform: DefinitionInstantiateTransform::default() } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DefinitionRenderPacketRequest {
    pub drawable_refs: Vec<String>,
    pub material_refs: Vec<String>,
    pub texture_refs: Vec<String>,
    pub policy: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DefinitionPhysicsDeclaration {
    pub collision_refs: Vec<String>,
    pub physics_refs: Vec<String>,
    pub policy: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionRuntimeTrace {
    pub schema: String,
    pub definition_ref: String,
    pub resolved_graph: ResolvedAssetGraphV1,
    pub entity_command: EntityCommandTrace,
    pub render_packet_request: DefinitionRenderPacketRequest,
    pub physics_declaration: DefinitionPhysicsDeclaration,
    pub apply_result: String,
    pub debug_log: Vec<String>,
}

impl Default for DefinitionRuntimeTrace {
    fn default() -> Self {
        Self {
            schema: "newengine.runtime.definition_instantiation_trace.v1".to_owned(),
            definition_ref: String::new(),
            resolved_graph: ResolvedAssetGraphV1::default(),
            entity_command: EntityCommandTrace::default(),
            render_packet_request: DefinitionRenderPacketRequest::default(),
            physics_declaration: DefinitionPhysicsDeclaration::default(),
            apply_result: String::new(),
            debug_log: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionInstance {
    pub definition_ref: String,
    pub stable_cache_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinitionRuntimeTraceComponent {
    pub trace: DefinitionRuntimeTrace,
}

pub fn apply_definition_instantiation(
    world: &mut World,
    parent: Option<EntityId>,
    definition_ref: String,
    transform: DefinitionInstantiateTransform,
    graph: ResolvedAssetGraphV1,
) -> (EntityId, DefinitionRuntimeTrace) {
    let entity_name = definition_entity_name(&definition_ref);
    let entity = spawn_named(world, entity_name.clone());
    if let Some(parent) = parent {
        let _ = newengine_transform::set_parent(world, entity, Some(parent));
    }
    let local_transform = Transform::from_yaw_pitch_roll(
        Vec3::new(transform.translation[0], transform.translation[1], transform.translation[2]),
        transform.rotation_ypr[0],
        transform.rotation_ypr[1],
        transform.rotation_ypr[2],
        Vec3::new(transform.scale[0], transform.scale[1], transform.scale[2]),
    );
    let _ = world.insert(entity, local_transform);
    let _ = world.insert(entity, DefinitionInstance { definition_ref: definition_ref.clone(), stable_cache_key: graph.stable_cache_key.clone() });

    let trace = build_definition_runtime_trace(&definition_ref, transform, graph, Some(entity));
    let _ = world.insert(entity, DefinitionRuntimeTraceComponent { trace: trace.clone() });
    for line in &trace.debug_log {
        log::debug!("{line}");
    }
    (entity, trace)
}

pub fn build_definition_runtime_trace(
    definition_ref: &str,
    transform: DefinitionInstantiateTransform,
    graph: ResolvedAssetGraphV1,
    entity: Option<EntityId>,
) -> DefinitionRuntimeTrace {
    let render_packet_request = DefinitionRenderPacketRequest {
        drawable_refs: graph_refs_by_render_edge(&graph, "ydd"),
        material_refs: graph_refs_by_render_edge(&graph, "nemat"),
        texture_refs: graph_refs_by_render_edge(&graph, "ytd"),
        policy: ".ytyp dependencies are graph dependencies only. Generic Definition instantiation emits render refs only for explicit render/spawn edge roles; domain systems consume sky/player/terrain metadata themselves.".to_owned(),
    };
    let physics_declaration = DefinitionPhysicsDeclaration {
        collision_refs: graph_refs_by_extension(&graph, "ycol"),
        physics_refs: graph_refs_by_extension(&graph, "ybn"),
        policy: "definition metadata produces physics declaration DTOs; definitions do not call physics backend".to_owned(),
    };
    let entity_name = definition_entity_name(definition_ref);
    let apply_result = match entity {
        Some(entity) => format!("spawned entity={entity:?}"),
        None => "dry_run:no_entity_applied".to_owned(),
    };
    let mut debug_log = vec![
        format!("definitions.runtime: definition_ref='{definition_ref}'"),
        "definitions.runtime: request assets.definitions.entry_v1 through engine.assets.definitions".to_owned(),
        format!("definitions.runtime: resolved graph nodes={} edges={} missing={} cache_key='{}'", graph.nodes.len(), graph.edges.len(), graph.missing_refs.len(), graph.stable_cache_key),
        format!("definitions.runtime: entity command EntityCommand::Spawn name='{entity_name}'"),
        format!("definitions.runtime: render packet request drawables={} materials={} textures={}", render_packet_request.drawable_refs.len(), render_packet_request.material_refs.len(), render_packet_request.texture_refs.len()),
        format!("definitions.runtime: physics declaration collision_refs={} physics_refs={}", physics_declaration.collision_refs.len(), physics_declaration.physics_refs.len()),
        format!("definitions.runtime: apply result {apply_result}"),
    ];
    debug_log.extend(graph.debug_log.iter().cloned());

    DefinitionRuntimeTrace {
        definition_ref: definition_ref.trim().replace('\\', "/"),
        resolved_graph: graph,
        entity_command: EntityCommandTrace { command: "EntityCommand::Spawn".to_owned(), entity_name, transform },
        render_packet_request,
        physics_declaration,
        apply_result,
        debug_log,
        ..Default::default()
    }
}

fn definition_entity_name(definition_ref: &str) -> String {
    let normalized = definition_ref.trim().replace('\\', "/");
    let entry = normalized.rsplit_once('@').map(|(_, entry)| entry).unwrap_or(normalized.as_str());
    format!("Definition/{entry}")
}

fn graph_refs_by_extension(graph: &ResolvedAssetGraphV1, extension: &str) -> Vec<String> {
    let suffix = format!(".{}", extension.trim_start_matches('.'));
    let mut refs = graph
        .nodes
        .iter()
        .map(|node| node.reference.clone())
        .filter(|reference| reference.split('@').next().unwrap_or(reference).to_ascii_lowercase().ends_with(&suffix))
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn graph_refs_by_render_edge(graph: &ResolvedAssetGraphV1, extension: &str) -> Vec<String> {
    let suffix = format!(".{}", extension.trim_start_matches('.'));
    let mut refs = graph
        .edges
        .iter()
        .filter(|edge| explicit_render_edge_role(&edge.kind))
        .map(|edge| edge.to_ref.clone())
        .filter(|reference| reference.split('@').next().unwrap_or(reference).to_ascii_lowercase().ends_with(&suffix))
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn explicit_render_edge_role(role: &str) -> bool {
    let role = role.trim().to_ascii_lowercase();
    role.starts_with("render/")
        || role.starts_with("spawn/")
        || role.starts_with("entity/")
        || role == "renderable"
        || role == "spawnable"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apply_instantiation_spawns_entity_and_trace_component() {
        let mut world = World::new();
        let graph = newengine_model_domain_api::AssetGraphResolver::resolve_root_ref("world/foo.ytyp@bar");
        let (entity, trace) = apply_definition_instantiation(
            &mut world,
            None,
            "world/foo.ytyp@bar".to_owned(),
            DefinitionInstantiateTransform::default(),
            graph,
        );
        assert!(world.get::<DefinitionInstance>(entity).is_some());
        assert!(world.get::<DefinitionRuntimeTraceComponent>(entity).is_some());
        assert!(trace.apply_result.contains("spawned entity"));
    }
}
