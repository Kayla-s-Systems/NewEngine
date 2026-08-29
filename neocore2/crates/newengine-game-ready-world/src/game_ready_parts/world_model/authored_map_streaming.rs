use super::super::*;
use super::streaming::{cancel_static_world_cell, enqueue_static_world_prefabs};
use super::{
    BOX_COLLISION_WORLD_PROXY, COLLISION_WORLD_PROXY, DYNAMIC_WORLD_PROXY, STATIC_WORLD_PROXY,
};
use crate::content::GameReadyAuthoredMapStreamingSpec;
use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionRefs {
    drawable_refs: Vec<String>,
    material_refs: Vec<String>,
    collision_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionModelExplanation {
    collision_policy: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionEntry {
    refs: ResolvedMapDefinitionRefs,
    semantic_tags: Vec<String>,
    model_explanation: ResolvedMapDefinitionModelExplanation,
}

type DefinitionCache = Arc<Mutex<BTreeMap<String, ResolvedMapDefinitionEntry>>>;
type CellLoadResult = Arc<Mutex<Option<Result<PreparedMapCell, String>>>>;

struct CellLoadJob {
    ticket: TaskTicket,
    result: CellLoadResult,
}

#[derive(Debug)]
struct PreparedMapCell {
    prefabs: Vec<GameReadyPrefabSpec>,
    placement_ids: Vec<String>,
    authored_placement_count: usize,
    metadata_only_count: usize,
}

pub(super) struct GameReadyAuthoredMapStreamingState {
    parent: EntityId,
    map_ref: String,
    logical_map_ref: String,
    index: newengine_assets_api::MapIndexV1,
    resident_radius: i32,
    unload_radius: i32,
    max_cells_per_tick: usize,
    max_pending_jobs: usize,
    loaded_cells: BTreeSet<newengine_assets_api::MapCellCoordV1>,
    placement_ids: BTreeMap<newengine_assets_api::MapCellCoordV1, Vec<String>>,
    pending_cells: VecDeque<newengine_assets_api::MapCellCoordV1>,
    pending_set: BTreeSet<newengine_assets_api::MapCellCoordV1>,
    load_jobs: BTreeMap<newengine_assets_api::MapCellCoordV1, CellLoadJob>,
    ready_cells: BTreeMap<newengine_assets_api::MapCellCoordV1, PreparedMapCell>,
    failed_cells: BTreeMap<newengine_assets_api::MapCellCoordV1, String>,
    definition_cache: DefinitionCache,
    last_center: Option<newengine_assets_api::MapCellCoordV1>,
    last_predicted_center: Option<newengine_assets_api::MapCellCoordV1>,
}

#[derive(Default)]
pub(super) struct GameReadyAuthoredMapCellRoots {
    roots: BTreeMap<newengine_assets_api::MapCellCoordV1, EntityId>,
}

/// Reference ownership for imported PrimitiveRegistry meshes contributed by streamed cells.
/// A primitive can be shared by many placements/cells; CPU/GPU eviction becomes legal only
/// after the last cell reference disappears.
#[derive(Default)]
pub(super) struct GameReadyAuthoredMapPrimitiveResidency {
    cell_primitives: BTreeMap<newengine_assets_api::MapCellCoordV1, BTreeSet<PrimitiveId>>,
    ref_counts: BTreeMap<PrimitiveId, u32>,
}

pub(super) fn record_static_world_primitive_residency(
    world: &mut newengine_ecs::World,
    prefab: &GameReadyPrefabSpec,
    decoded: &[super::super::foliage::DecodedPrefabMeshPart],
) {
    let Some(coord) = prefab.authored_cell else {
        return;
    };
    if world
        .resource::<GameReadyAuthoredMapPrimitiveResidency>()
        .is_none()
    {
        world.insert_resource(GameReadyAuthoredMapPrimitiveResidency::default());
    }
    let Some(residency) = world.resource_mut::<GameReadyAuthoredMapPrimitiveResidency>() else {
        return;
    };
    let cell = residency.cell_primitives.entry(coord).or_default();
    for part in decoded {
        if cell.insert(part.primitive_id) {
            *residency.ref_counts.entry(part.primitive_id).or_default() += 1;
        }
    }
}

fn release_cell_primitive_residency(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    coord: newengine_assets_api::MapCellCoordV1,
) -> usize {
    let candidates = {
        let Some(residency) = world.resource_mut::<GameReadyAuthoredMapPrimitiveResidency>() else {
            return 0;
        };
        let Some(ids) = residency.cell_primitives.remove(&coord) else {
            return 0;
        };
        let mut candidates = Vec::new();
        for id in ids {
            match residency.ref_counts.get_mut(&id) {
                Some(count) if *count > 1 => *count -= 1,
                Some(_) => {
                    residency.ref_counts.remove(&id);
                    candidates.push(id);
                }
                None => candidates.push(id),
            }
        }
        candidates
    };
    if candidates.is_empty() {
        return 0;
    }

    // Protect any non-cell/legacy entity that still references the same primitive id.
    let active = world
        .query::<Primitive>()
        .map(|(_, primitive)| primitive.id)
        .collect::<BTreeSet<_>>();
    if world
        .resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
        .is_none()
    {
        world.insert_resource(
            newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue::default(),
        );
    }
    let mut released = 0usize;
    for id in candidates {
        if active.contains(&id) || !prims.unregister_runtime_mesh(id) {
            continue;
        }
        if let Some(queue) =
            world.resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
        {
            queue.enqueue(id);
        }
        released = released.saturating_add(1);
    }
    released
}

#[inline]
pub(super) fn static_world_parent_for_prefab(
    world: &newengine_ecs::World,
    default_parent: EntityId,
    prefab: &GameReadyPrefabSpec,
) -> EntityId {
    prefab
        .authored_cell
        .and_then(|coord| {
            world
                .resource::<GameReadyAuthoredMapCellRoots>()
                .and_then(|registry| registry.roots.get(&coord).copied())
        })
        .filter(|entity| world.exists(*entity))
        .unwrap_or(default_parent)
}

fn spawn_cell_root(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
) -> EntityId {
    if let Some(existing) = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).copied())
        .filter(|entity| world.exists(*entity))
    {
        return existing;
    }

    let entity = spawn_named(world, format!("World/Cells/{}/{}", coord.x, coord.z));
    let _ = world.insert(entity, Transform::default());
    let _ = set_parent(world, entity, Some(parent));
    if world.resource::<GameReadyAuthoredMapCellRoots>().is_none() {
        world.insert_resource(GameReadyAuthoredMapCellRoots::default());
    }
    if let Some(registry) = world.resource_mut::<GameReadyAuthoredMapCellRoots>() {
        registry.roots.insert(coord, entity);
    }
    newengine_ulog_api::ulog::debug!(
        "authored map cell root created map='{}' cell={},{} entity={:?}",
        map_ref,
        coord.x,
        coord.z,
        entity,
    );
    entity
}

fn remove_cell_root(
    world: &mut newengine_ecs::World,
    coord: newengine_assets_api::MapCellCoordV1,
) -> Option<EntityId> {
    world
        .resource_mut::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.remove(&coord))
}

#[inline]
fn cell_distance(
    a: newengine_assets_api::MapCellCoordV1,
    b: newengine_assets_api::MapCellCoordV1,
) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

fn append_existing_cells(
    index: &newengine_assets_api::MapIndexV1,
    center: newengine_assets_api::MapCellCoordV1,
    radius: i32,
    output: &mut BTreeSet<newengine_assets_api::MapCellCoordV1>,
) {
    let radius = radius.max(0);
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let coord = newengine_assets_api::MapCellCoordV1::new(
                center.x.saturating_add(dx),
                center.z.saturating_add(dz),
            );
            if index.cell(coord).is_some() {
                output.insert(coord);
            }
        }
    }
}

fn prediction_for_player(
    state: &GameReadyAuthoredMapStreamingState,
    position: Vec3,
    velocity: Vec3,
) -> Option<(
    newengine_assets_api::MapCellCoordV1,
    newengine_assets_api::MapCellCoordV1,
    f32,
    f32,
    i32,
)> {
    let center = state
        .index
        .world_to_cell([position.x, position.y, position.z])?;
    let horizontal = Vec3::new(velocity.x, 0.0, velocity.z);
    let speed = horizontal.length();
    let read_ahead_sec =
        crate::env_config::var_f32("NEWENGINE_AUTHORED_MAP_READ_AHEAD_SEC", 0.75, 0.0, 3.0);
    let predicted_position = position + horizontal * read_ahead_sec;
    let raw_prediction = state
        .index
        .world_to_cell([
            predicted_position.x,
            predicted_position.y,
            predicted_position.z,
        ])
        .unwrap_or(center);
    let default_max_cells = state.resident_radius.saturating_add(1).clamp(1, 4);
    let max_read_ahead_cells = crate::env_config::var_i32(
        "NEWENGINE_AUTHORED_MAP_MAX_READ_AHEAD_CELLS",
        default_max_cells,
        0,
        8,
    );
    let dx = (raw_prediction.x - center.x).clamp(-max_read_ahead_cells, max_read_ahead_cells);
    let dz = (raw_prediction.z - center.z).clamp(-max_read_ahead_cells, max_read_ahead_cells);
    let predicted = newengine_assets_api::MapCellCoordV1::new(
        center.x.saturating_add(dx),
        center.z.saturating_add(dz),
    );
    Some((
        center,
        predicted,
        speed,
        read_ahead_sec,
        max_read_ahead_cells,
    ))
}

pub(in super::super) fn begin_authored_map_streaming(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    spec: Option<&GameReadyAuthoredMapStreamingSpec>,
) {
    let Some(spec) = spec else {
        return;
    };
    let loaded_cells = spec.initial_cells.iter().copied().collect::<BTreeSet<_>>();
    world.insert_resource(GameReadyAuthoredMapCellRoots::default());
    world.insert_resource(GameReadyAuthoredMapPrimitiveResidency::default());
    for coord in spec.initial_cells.iter().copied() {
        let _ = spawn_cell_root(world, parent, &spec.map_ref, coord);
    }
    let logical_map_ref = spec
        .map_ref
        .split('@')
        .next()
        .unwrap_or(&spec.map_ref)
        .to_owned();
    let max_pending_jobs = crate::env_config::var_usize(
        "NEWENGINE_AUTHORED_MAP_MAX_PENDING_JOBS",
        spec.max_cells_per_tick.saturating_mul(4).max(4),
        1,
        64,
    );
    world.insert_resource(GameReadyAuthoredMapStreamingState {
        parent,
        map_ref: spec.map_ref.clone(),
        logical_map_ref,
        index: spec.index.clone(),
        resident_radius: spec.resident_radius,
        unload_radius: spec.unload_radius,
        max_cells_per_tick: spec.max_cells_per_tick,
        max_pending_jobs,
        loaded_cells,
        placement_ids: spec.initial_placement_ids.clone(),
        pending_cells: VecDeque::new(),
        pending_set: BTreeSet::new(),
        load_jobs: BTreeMap::new(),
        ready_cells: BTreeMap::new(),
        failed_cells: BTreeMap::new(),
        definition_cache: Arc::new(Mutex::new(BTreeMap::new())),
        last_center: None,
        last_predicted_center: None,
    });
    newengine_ulog_api::ulog::info!(
        "authored map streaming initialized map='{}' cells_total={} cells_resident={} resident_radius={} unload_radius={} max_cells_per_tick={} max_pending_jobs={} policy='YMAP index resident; async cell payload/definition prepare; cell-root ownership'",
        spec.map_ref,
        spec.index.cells.len(),
        spec.initial_cells.len(),
        spec.resident_radius,
        spec.unload_radius,
        spec.max_cells_per_tick,
        max_pending_jobs,
    );
}

fn load_cell(
    map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
) -> Result<newengine_assets_api::MapResolvedCellV2, String> {
    let request = serde_json::to_vec(&newengine_assets_api::MapCellRequestV1 {
        map_ref: map_ref.to_owned(),
        coord,
    })
    .map_err(|error| format!("map cell request encode failed: {error}"))?;
    let bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        newengine_assets_api::maps_method::CELL_V2,
        &request,
    )
    .map_err(|error| {
        format!(
            "map cell request failed map='{map_ref}' cell={},{} err='{error}'",
            coord.x, coord.z
        )
    })?
    .ok_or_else(|| {
        format!(
            "engine.assets.maps unavailable map='{map_ref}' cell={},{}",
            coord.x, coord.z
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "invalid MapResolvedCellV2 map='{map_ref}' cell={},{} err='{error}'",
            coord.x, coord.z
        )
    })
}

fn resolve_definition(
    cache: &DefinitionCache,
    definition_ref: &str,
) -> Result<ResolvedMapDefinitionEntry, String> {
    if let Some(existing) = cache.lock().get(definition_ref).cloned() {
        return Ok(existing);
    }
    let payload = serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref }))
        .map_err(|error| format!("definition request encode failed: {error}"))?;
    let bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets_api::definitions_method::ENTRY_JSON_V1,
        &payload,
    )
    .map_err(|error| {
        format!("definition request failed definition_ref='{definition_ref}' err='{error}'")
    })?
    .ok_or_else(|| {
        format!("engine.assets.definitions unavailable definition_ref='{definition_ref}'")
    })?;
    let parsed: ResolvedMapDefinitionEntry = serde_json::from_slice(&bytes).map_err(|error| {
        format!("invalid definition DTO definition_ref='{definition_ref}' err='{error}'")
    })?;
    let mut locked = cache.lock();
    Ok(locked
        .entry(definition_ref.to_owned())
        .or_insert_with(|| parsed.clone())
        .clone())
}

fn placement_is_spawn(placement: &newengine_assets_api::MapPlacementV1) -> bool {
    placement.tags.iter().any(|tag| {
        matches!(
            tag.trim().to_ascii_lowercase().as_str(),
            "player_spawn" | "info_player_start" | "spawn.player"
        )
    }) || matches!(
        placement.apply_mode.trim().to_ascii_lowercase().as_str(),
        "player_spawn" | "info_player_start"
    )
}

fn cell_prefabs(
    logical_map_ref: &str,
    resolved: &newengine_assets_api::MapResolvedCellV2,
    definition_cache: &DefinitionCache,
) -> Result<(Vec<GameReadyPrefabSpec>, Vec<String>, usize), String> {
    let mut prefabs = Vec::new();
    let mut placement_ids = Vec::new();
    let mut metadata_only_count = 0usize;
    for placement in resolved
        .cell
        .placements
        .iter()
        .filter(|placement| placement.enabled)
    {
        placement_ids.push(placement.id.clone());
        if placement_is_spawn(placement) {
            continue;
        }
        if placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("metadata_only")
        {
            // Root map metadata is the correct home for global domain configuration. A streamed
            // metadata-only placement cannot safely mutate already-running domain state here.
            metadata_only_count = metadata_only_count.saturating_add(1);
            continue;
        }
        let definition = resolve_definition(definition_cache, &placement.definition_ref)?;
        let drawable_ref = definition
            .refs
            .drawable_refs
            .first()
            .cloned()
            .ok_or_else(|| {
                format!(
                    "streamed placement '{}' definition_ref='{}' has no drawable_refs",
                    placement.id, placement.definition_ref
                )
            })?;
        let material_ref = definition
            .refs
            .material_refs
            .first()
            .cloned()
            .unwrap_or_default();
        let position = Vec3::new(
            placement.transform.position[0],
            placement.transform.position[1],
            placement.transform.position[2],
        );
        let rotation_ypr = Vec3::new(
            placement.transform.rotation_ypr[0],
            placement.transform.rotation_ypr[1],
            placement.transform.rotation_ypr[2],
        );
        let scale = Vec3::new(
            placement.transform.scale[0],
            placement.transform.scale[1],
            placement.transform.scale[2],
        );
        let dynamic_physics = placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("dynamic_physics");
        let collision_only = placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("collision_only")
            || placement
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("collision_only"));

        if !collision_only {
            prefabs.push(GameReadyPrefabSpec {
                id: placement.id.clone(),
                authored_map_ref: logical_map_ref.to_owned(),
                authored_placement_id: placement.id.clone(),
                authored_cell: Some(resolved.cell.coord),
                authored_discrete_placement: true,
                authored_primary: true,
                source: drawable_ref.clone(),
                proxy: if dynamic_physics {
                    DYNAMIC_WORLD_PROXY.to_owned()
                } else {
                    STATIC_WORLD_PROXY.to_owned()
                },
                material: material_ref,
                enabled: true,
                position,
                rotation_ypr,
                scale,
            });
        }

        let collision_policy = definition.model_explanation.collision_policy.trim();
        let has_collision = !definition.refs.collision_refs.is_empty()
            || definition
                .semantic_tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("collision"))
            || matches!(
                collision_policy.to_ascii_lowercase().as_str(),
                "static_mesh" | "triangle_mesh" | "mesh" | "box"
            );
        if has_collision && !dynamic_physics {
            let collision_source = definition
                .refs
                .collision_refs
                .first()
                .cloned()
                .unwrap_or(drawable_ref);
            prefabs.push(GameReadyPrefabSpec {
                id: if collision_only {
                    placement.id.clone()
                } else {
                    format!("{}#collision", placement.id)
                },
                authored_map_ref: logical_map_ref.to_owned(),
                authored_placement_id: placement.id.clone(),
                authored_cell: Some(resolved.cell.coord),
                authored_discrete_placement: true,
                authored_primary: false,
                source: collision_source,
                proxy: if collision_policy.eq_ignore_ascii_case("box") {
                    BOX_COLLISION_WORLD_PROXY.to_owned()
                } else {
                    COLLISION_WORLD_PROXY.to_owned()
                },
                material: String::new(),
                enabled: true,
                position,
                rotation_ypr,
                scale,
            });
        } else if collision_only {
            return Err(format!(
                "streamed collision_only placement '{}' definition_ref='{}' declares no collision",
                placement.id, placement.definition_ref
            ));
        }
    }
    Ok((prefabs, placement_ids, metadata_only_count))
}

fn prepare_cell(
    map_ref: &str,
    logical_map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
    definition_cache: &DefinitionCache,
) -> Result<PreparedMapCell, String> {
    let resolved = load_cell(map_ref, coord)?;
    let authored_placement_count = resolved.cell.placements.len();
    let (prefabs, placement_ids, metadata_only_count) =
        cell_prefabs(logical_map_ref, &resolved, definition_cache)?;
    Ok(PreparedMapCell {
        prefabs,
        placement_ids,
        authored_placement_count,
        metadata_only_count,
    })
}

fn cell_load_concurrency(
    state: &GameReadyAuthoredMapStreamingState,
    thread_pool: &ThreadPoolHandle,
) -> usize {
    let adaptive_default = thread_pool
        .worker_threads()
        .saturating_sub(1)
        .clamp(1, state.max_cells_per_tick.max(1).min(4)) as u32;
    crate::env_config::var_u32("NEWENGINE_AUTHORED_MAP_CELL_JOBS", adaptive_default, 1, 8) as usize
}

fn submit_cell_jobs(
    state: &mut GameReadyAuthoredMapStreamingState,
    thread_pool: &ThreadPoolHandle,
) {
    let concurrency = cell_load_concurrency(state, thread_pool).min(state.max_pending_jobs);
    let free_slots = concurrency.saturating_sub(state.load_jobs.len());
    if free_slots == 0 {
        return;
    }
    for _ in 0..free_slots {
        let Some(coord) = state.pending_cells.pop_front() else {
            break;
        };
        state.pending_set.remove(&coord);
        if state.loaded_cells.contains(&coord)
            || state.load_jobs.contains_key(&coord)
            || state.ready_cells.contains_key(&coord)
        {
            continue;
        }
        let map_ref = state.map_ref.clone();
        let logical_map_ref = state.logical_map_ref.clone();
        let definition_cache = Arc::clone(&state.definition_cache);
        let result = Arc::new(Mutex::new(None));
        let result_out = Arc::clone(&result);
        let request = TaskRequest::new("authored.map.cell.prepare")
            .with_source("scene.bridge.game-ready")
            .with_owner("engine.scene")
            .with_category("world-streaming")
            .with_lane(TaskLane::AssetIo)
            .with_priority(TaskPriority::Interactive)
            .with_task_id(format!(
                "scene.authored-map.cell.{}.{}.{:016x}",
                coord.x,
                coord.z,
                newengine_primitives::fnv1a_64(&map_ref)
            ));
        let host_context = newengine_plugin_host::current_host_context();
        let ticket = thread_pool.submit_request(request, move || {
            let prepared = newengine_plugin_host::with_host_context(&host_context, || {
                prepare_cell(&map_ref, &logical_map_ref, coord, &definition_cache)
            });
            *result_out.lock() = Some(prepared);
        });
        state
            .load_jobs
            .insert(coord, CellLoadJob { ticket, result });
    }
}

fn poll_cell_jobs(state: &mut GameReadyAuthoredMapStreamingState) {
    let complete = state
        .load_jobs
        .iter()
        .filter(|(_, job)| job.ticket.is_complete())
        .map(|(coord, _)| *coord)
        .collect::<Vec<_>>();
    for coord in complete {
        let Some(job) = state.load_jobs.remove(&coord) else {
            continue;
        };
        let job_result = job.result.lock().take();
        match job_result {
            Some(Ok(prepared)) => {
                state.failed_cells.remove(&coord);
                state.ready_cells.insert(coord, prepared);
            }
            Some(Err(error)) => {
                state.failed_cells.insert(coord, error);
            }
            None => {
                state.failed_cells.insert(
                    coord,
                    "authored map cell task completed without result".to_owned(),
                );
            }
        }
    }
}

fn prepare_cells_synchronously(state: &mut GameReadyAuthoredMapStreamingState) {
    let budget = state.max_cells_per_tick.max(1);
    for _ in 0..budget {
        if state.ready_cells.len() >= budget {
            break;
        }
        let Some(coord) = state.pending_cells.pop_front() else {
            break;
        };
        state.pending_set.remove(&coord);
        match prepare_cell(
            &state.map_ref,
            &state.logical_map_ref,
            coord,
            &state.definition_cache,
        ) {
            Ok(prepared) => {
                state.ready_cells.insert(coord, prepared);
            }
            Err(error) => {
                state.failed_cells.insert(coord, error);
            }
        }
    }
}

fn unload_cell(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    state: &mut GameReadyAuthoredMapStreamingState,
    coord: newengine_assets_api::MapCellCoordV1,
) -> usize {
    let placement_count = state
        .placement_ids
        .remove(&coord)
        .map_or(0, |ids| ids.len());
    let cancelled = cancel_static_world_cell(world, &state.logical_map_ref, coord);
    let removed = remove_cell_root(world, coord)
        .map(|root| newengine_transform::despawn_hierarchy(world, root))
        .unwrap_or(0);
    let primitive_releases = release_cell_primitive_residency(world, prims, coord);
    state.loaded_cells.remove(&coord);
    state.ready_cells.remove(&coord);
    state.failed_cells.remove(&coord);
    newengine_ulog_api::ulog::debug!(
        "authored map cell unloaded map='{}' cell={},{} placements={} cancelled_pending={} entities_removed={} primitive_releases={} resident_cells={}",
        state.map_ref,
        coord.x,
        coord.z,
        placement_count,
        cancelled,
        removed,
        primitive_releases,
        state.loaded_cells.len(),
    );
    removed
}

fn replan_residency(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    state: &mut GameReadyAuthoredMapStreamingState,
    center: newengine_assets_api::MapCellCoordV1,
    predicted_center: newengine_assets_api::MapCellCoordV1,
) {
    let predictive_radius = crate::env_config::var_i32(
        "NEWENGINE_AUTHORED_MAP_PREDICT_RADIUS",
        1.min(state.resident_radius),
        0,
        state.resident_radius.max(0),
    );
    let mut desired = BTreeSet::new();
    append_existing_cells(&state.index, center, state.resident_radius, &mut desired);
    if predicted_center != center && predictive_radius > 0 {
        let mut predicted = BTreeSet::new();
        append_existing_cells(
            &state.index,
            predicted_center,
            predictive_radius,
            &mut predicted,
        );
        desired.extend(
            predicted
                .into_iter()
                .filter(|coord| cell_distance(*coord, center) <= state.unload_radius),
        );
    }

    let stale = state
        .loaded_cells
        .iter()
        .copied()
        .filter(|coord| cell_distance(*coord, center) > state.unload_radius)
        .collect::<Vec<_>>();
    for coord in stale {
        unload_cell(world, prims, state, coord);
    }

    let stale_jobs = state
        .load_jobs
        .keys()
        .copied()
        .filter(|coord| cell_distance(*coord, center) > state.unload_radius)
        .collect::<Vec<_>>();
    for coord in stale_jobs {
        if let Some(job) = state.load_jobs.remove(&coord) {
            let _ = job.ticket.cancel();
        }
    }
    state
        .ready_cells
        .retain(|coord, _| cell_distance(*coord, center) <= state.unload_radius);
    state
        .failed_cells
        .retain(|coord, _| cell_distance(*coord, center) <= state.unload_radius);

    let mut candidates = desired
        .into_iter()
        .filter(|coord| !state.loaded_cells.contains(coord))
        .filter(|coord| !state.pending_set.contains(coord))
        .filter(|coord| !state.load_jobs.contains_key(coord))
        .filter(|coord| !state.ready_cells.contains_key(coord))
        .collect::<Vec<_>>();
    // Re-entering/changing focus is an explicit retry opportunity for failed cells.
    for coord in &candidates {
        state.failed_cells.remove(coord);
    }
    candidates.sort_by_key(|coord| {
        let primary_distance = cell_distance(*coord, center);
        let predicted_distance = cell_distance(*coord, predicted_center);
        let predictive_only = primary_distance > state.resident_radius;
        (
            usize::from(predictive_only),
            if predictive_only {
                predicted_distance
            } else {
                primary_distance
            },
            coord.x,
            coord.z,
        )
    });
    for coord in candidates {
        state.pending_set.insert(coord);
        state.pending_cells.push_back(coord);
    }
    state
        .pending_cells
        .retain(|coord| cell_distance(*coord, center) <= state.unload_radius);
    state.pending_set = state.pending_cells.iter().copied().collect();
}

fn next_ready_coord(
    state: &GameReadyAuthoredMapStreamingState,
    center: newengine_assets_api::MapCellCoordV1,
    predicted_center: newengine_assets_api::MapCellCoordV1,
) -> Option<newengine_assets_api::MapCellCoordV1> {
    state.ready_cells.keys().copied().min_by_key(|coord| {
        let primary_distance = cell_distance(*coord, center);
        let predictive_only = primary_distance > state.resident_radius;
        (
            usize::from(predictive_only),
            if predictive_only {
                cell_distance(*coord, predicted_center)
            } else {
                primary_distance
            },
            coord.x,
            coord.z,
        )
    })
}

fn admit_ready_cells(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    state: &mut GameReadyAuthoredMapStreamingState,
    center: newengine_assets_api::MapCellCoordV1,
    predicted_center: newengine_assets_api::MapCellCoordV1,
) -> usize {
    let mut admitted = 0usize;
    for _ in 0..state.max_cells_per_tick {
        let Some(coord) = next_ready_coord(state, center, predicted_center) else {
            break;
        };
        let Some(prepared) = state.ready_cells.remove(&coord) else {
            continue;
        };
        if cell_distance(coord, center) > state.unload_radius {
            continue;
        }
        let _ = spawn_cell_root(world, state.parent, &state.map_ref, coord);
        enqueue_static_world_prefabs(world, mats, state.parent, &prepared.prefabs);
        state.loaded_cells.insert(coord);
        state.placement_ids.insert(coord, prepared.placement_ids);
        admitted = admitted.saturating_add(1);
        if prepared.metadata_only_count > 0 {
            newengine_ulog_api::ulog::warn!(
                "authored map streamed cell contains metadata-only placements map='{}' cell={},{} count={} policy='global domain metadata belongs in map index/startup cell; runtime mutation skipped'",
                state.map_ref,
                coord.x,
                coord.z,
                prepared.metadata_only_count,
            );
        }
        newengine_ulog_api::ulog::info!(
            "authored map cell resident map='{}' cell={},{} placements={} prefabs={} resident_cells={} center={},{} predicted={},{}",
            state.map_ref,
            coord.x,
            coord.z,
            prepared.authored_placement_count,
            prepared.prefabs.len(),
            state.loaded_cells.len(),
            center.x,
            center.z,
            predicted_center.x,
            predicted_center.z,
        );
    }
    admitted
}

pub(crate) fn tick_authored_map_streaming(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    let Some(player) = newengine_engine_runtime::gameplay::first_player(world) else {
        return;
    };
    let player_position = world
        .get::<Transform>(player)
        .map(|transform| transform.position)
        .unwrap_or(Vec3::ZERO);
    let player_velocity = world
        .get::<newengine_sim::Velocity>(player)
        .map(|velocity| velocity.0)
        .unwrap_or(Vec3::ZERO);
    let Some(mut state) = world.remove_resource::<GameReadyAuthoredMapStreamingState>() else {
        return;
    };
    let Some((center, predicted_center, speed, read_ahead_sec, max_read_ahead_cells)) =
        prediction_for_player(&state, player_position, player_velocity)
    else {
        world.insert_resource(state);
        return;
    };

    let focus_changed =
        state.last_center != Some(center) || state.last_predicted_center != Some(predicted_center);
    if focus_changed {
        replan_residency(world, prims, &mut state, center, predicted_center);
        state.last_center = Some(center);
        state.last_predicted_center = Some(predicted_center);
    }

    poll_cell_jobs(&mut state);
    if let Some(thread_pool) = thread_pool {
        submit_cell_jobs(&mut state, thread_pool);
        poll_cell_jobs(&mut state);
    } else {
        prepare_cells_synchronously(&mut state);
    }
    let admitted = admit_ready_cells(world, mats, &mut state, center, predicted_center);

    if focus_changed || admitted > 0 {
        newengine_ulog_api::ulog::debug!(
            "authored map streaming tick map='{}' center={},{} predicted={},{} speed_mps={:.2} read_ahead_sec={:.2} max_read_ahead_cells={} admitted={} resident={} queued={} jobs={} ready={} failed={}",
            state.map_ref,
            center.x,
            center.z,
            predicted_center.x,
            predicted_center.z,
            speed,
            read_ahead_sec,
            max_read_ahead_cells,
            admitted,
            state.loaded_cells.len(),
            state.pending_cells.len(),
            state.load_jobs.len(),
            state.ready_cells.len(),
            state.failed_cells.len(),
        );
    }
    world.insert_resource(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_with_cells(coords: &[(i32, i32)]) -> newengine_assets_api::MapIndexV1 {
        let mut index = newengine_assets_api::MapIndexV1 {
            map_id: "test".to_owned(),
            cell_size: 64.0,
            cells: coords
                .iter()
                .map(|(x, z)| {
                    newengine_assets_api::MapCellRefV1::canonical(
                        newengine_assets_api::MapCellCoordV1::new(*x, *z),
                    )
                })
                .collect(),
            ..Default::default()
        };
        index.normalize();
        index
    }

    #[test]
    fn desired_cell_generation_scales_with_radius_not_world_cell_count() {
        let index = index_with_cells(&[(-100, -100), (-1, -1), (0, 0), (1, 0), (1, 1), (100, 100)]);
        let mut desired = BTreeSet::new();
        append_existing_cells(
            &index,
            newengine_assets_api::MapCellCoordV1::new(0, 0),
            1,
            &mut desired,
        );
        assert!(desired.contains(&newengine_assets_api::MapCellCoordV1::new(0, 0)));
        assert!(desired.contains(&newengine_assets_api::MapCellCoordV1::new(1, 0)));
        assert!(desired.contains(&newengine_assets_api::MapCellCoordV1::new(1, 1)));
        assert!(!desired.contains(&newengine_assets_api::MapCellCoordV1::new(100, 100)));
        assert_eq!(desired.len(), 4);
    }

    #[test]
    fn cell_root_owns_prefab_parent_and_despawns_as_one_subtree() {
        let mut world = newengine_ecs::World::new();
        let terrain = world.spawn();
        let _ = world.insert(terrain, Transform::default());
        world.insert_resource(GameReadyAuthoredMapCellRoots::default());
        let coord = newengine_assets_api::MapCellCoordV1::new(2, -3);
        let root = spawn_cell_root(&mut world, terrain, "maps/test.ymap@map", coord);
        let child = world.spawn();
        let _ = world.insert(child, Transform::default());
        let _ = set_parent(&mut world, child, Some(root));
        let prefab = GameReadyPrefabSpec {
            id: "p".to_owned(),
            authored_map_ref: "maps/test.ymap".to_owned(),
            authored_placement_id: "p".to_owned(),
            authored_cell: Some(coord),
            authored_discrete_placement: true,
            authored_primary: true,
            source: "models/test.ydd".to_owned(),
            proxy: STATIC_WORLD_PROXY.to_owned(),
            material: String::new(),
            enabled: true,
            position: Vec3::ZERO,
            rotation_ypr: Vec3::ZERO,
            scale: Vec3::ONE,
        };
        assert_eq!(
            static_world_parent_for_prefab(&world, terrain, &prefab),
            root
        );
        assert_eq!(newengine_transform::despawn_hierarchy(&mut world, root), 2);
        assert!(!world.exists(root));
        assert!(!world.exists(child));
        assert!(world.exists(terrain));
    }

    #[test]
    fn shared_cell_mesh_is_evicted_only_after_last_cell_reference() {
        let mut world = newengine_ecs::World::new();
        let mut prims = PrimitiveRegistry::new();
        let id = PrimitiveId::new(0xabc0_1234);
        prims.register_mesh(
            id,
            "shared-cell-mesh",
            PrimitiveMesh {
                vertices: vec![
                    PrimitiveVertex {
                        pos: [0.0, 0.0, 0.0],
                        nrm: [0.0, 1.0, 0.0],
                        uv: [0.0, 0.0],
                    },
                    PrimitiveVertex {
                        pos: [1.0, 0.0, 0.0],
                        nrm: [0.0, 1.0, 0.0],
                        uv: [1.0, 0.0],
                    },
                    PrimitiveVertex {
                        pos: [0.0, 0.0, 1.0],
                        nrm: [0.0, 1.0, 0.0],
                        uv: [0.0, 1.0],
                    },
                ],
                indices: vec![0, 1, 2],
                bounds_center: Vec3::new(0.5, 0.0, 0.5),
                bounds_radius: 1.0,
            },
        );
        let first = newengine_assets_api::MapCellCoordV1::new(0, 0);
        let second = newengine_assets_api::MapCellCoordV1::new(1, 0);
        world.insert_resource(GameReadyAuthoredMapPrimitiveResidency {
            cell_primitives: BTreeMap::from([
                (first, BTreeSet::from([id])),
                (second, BTreeSet::from([id])),
            ]),
            ref_counts: BTreeMap::from([(id, 2)]),
        });

        assert_eq!(
            release_cell_primitive_residency(&mut world, &mut prims, first),
            0
        );
        assert!(prims.is_registered(id));
        assert!(world
            .resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
            .is_none());

        assert_eq!(
            release_cell_primitive_residency(&mut world, &mut prims, second),
            1
        );
        assert!(!prims.is_registered(id));
        let queue = world
            .resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
            .expect("GPU eviction queue");
        assert_eq!(queue.len(), 1);
    }
}
