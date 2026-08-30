use super::super::*;
#[path = "authored_map_streaming/prepare.rs"]
mod prepare;
use prepare::{prepare_cell, DefinitionCache};

use super::streaming::{cancel_static_world_cell_domain, enqueue_static_world_prefabs};
use super::{
    BOX_COLLISION_WORLD_PROXY, COLLISION_WORLD_PROXY, DYNAMIC_WORLD_PROXY, STATIC_WORLD_PROXY,
};
use crate::content::GameReadyAuthoredMapStreamingSpec;
use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use parking_lot::Mutex;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

type CellCoord = newengine_assets_api::MapCellCoordV1;
type CellLoadResult = Arc<Mutex<Option<Result<PreparedMapCell, String>>>>;

struct CellLoadJob {
    ticket: TaskTicket,
    result: CellLoadResult,
}

#[derive(Clone, Debug)]
struct PreparedMapCell {
    render_prefabs: Vec<GameReadyPrefabSpec>,
    simulation_prefabs: Vec<GameReadyPrefabSpec>,
    placement_ids: Vec<String>,
    authored_placement_count: usize,
    metadata_only_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AuthoredCellDomain {
    Render,
    Simulation,
}

impl AuthoredCellDomain {
    #[inline]
    fn label(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Simulation => "simulation",
        }
    }
}

#[inline]
pub(super) fn static_world_prefab_domain(prefab: &GameReadyPrefabSpec) -> AuthoredCellDomain {
    let proxy = prefab.proxy.trim();
    if proxy.eq_ignore_ascii_case(COLLISION_WORLD_PROXY)
        || proxy.eq_ignore_ascii_case(BOX_COLLISION_WORLD_PROXY)
        || proxy.eq_ignore_ascii_case(DYNAMIC_WORLD_PROXY)
    {
        AuthoredCellDomain::Simulation
    } else {
        AuthoredCellDomain::Render
    }
}

pub(super) struct GameReadyAuthoredMapStreamingState {
    parent: EntityId,
    map_ref: String,
    logical_map_ref: String,
    index: newengine_assets_api::MapIndexV1,

    render_radius: i32,
    simulation_radius: i32,
    render_unload_radius: i32,
    simulation_unload_radius: i32,
    max_cells_per_tick: usize,
    max_pending_jobs: usize,

    // Runtime tuning is immutable for the lifetime of this streaming instance.
    // Do not query environment variables on the frame path.
    read_ahead_sec: f32,
    max_read_ahead_cells: i32,
    render_predict_radius: i32,
    simulation_predict_radius: i32,
    cell_jobs_limit: usize,

    render_cells: BTreeSet<CellCoord>,
    simulation_cells: BTreeSet<CellCoord>,
    desired_render: BTreeSet<CellCoord>,
    desired_simulation: BTreeSet<CellCoord>,

    resident_prepared: BTreeMap<CellCoord, PreparedMapCell>,
    placement_ids: BTreeMap<CellCoord, Vec<String>>,

    pending_cells: VecDeque<CellCoord>,
    pending_set: BTreeSet<CellCoord>,
    load_jobs: BTreeMap<CellCoord, CellLoadJob>,
    ready_cells: BTreeMap<CellCoord, PreparedMapCell>,
    failed_cells: BTreeMap<CellCoord, String>,
    definition_cache: DefinitionCache,

    last_center: Option<CellCoord>,
    last_predicted_center: Option<CellCoord>,
}

#[derive(Clone, Copy, Debug)]
struct AuthoredCellRoots {
    cell: EntityId,
    render: Option<EntityId>,
    simulation: Option<EntityId>,
}

#[derive(Default)]
pub(super) struct GameReadyAuthoredMapCellRoots {
    roots: BTreeMap<CellCoord, AuthoredCellRoots>,
}

/// Reference ownership for runtime meshes contributed by streamed authored cells.
/// Domain is part of the key because render and simulation have independent residency.
#[derive(Default)]
pub(super) struct GameReadyAuthoredMapPrimitiveResidency {
    cell_primitives: BTreeMap<(CellCoord, AuthoredCellDomain), BTreeSet<PrimitiveId>>,
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
    let domain = static_world_prefab_domain(prefab);
    let Some(residency) = world.resource_mut::<GameReadyAuthoredMapPrimitiveResidency>() else {
        return;
    };
    let cell = residency
        .cell_primitives
        .entry((coord, domain))
        .or_default();
    for part in decoded {
        if cell.insert(part.primitive_id) {
            *residency.ref_counts.entry(part.primitive_id).or_default() += 1;
        }
    }
}

fn take_primitive_release_candidates(
    world: &mut newengine_ecs::World,
    coord: CellCoord,
    domain: AuthoredCellDomain,
    output: &mut BTreeSet<PrimitiveId>,
) {
    let Some(residency) = world.resource_mut::<GameReadyAuthoredMapPrimitiveResidency>() else {
        return;
    };
    let Some(ids) = residency.cell_primitives.remove(&(coord, domain)) else {
        return;
    };
    for id in ids {
        match residency.ref_counts.get_mut(&id) {
            Some(count) if *count > 1 => *count -= 1,
            Some(_) => {
                residency.ref_counts.remove(&id);
                output.insert(id);
            }
            None => {
                output.insert(id);
            }
        }
    }
}

fn finalize_primitive_releases(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    candidates: BTreeSet<PrimitiveId>,
) -> usize {
    if candidates.is_empty() {
        return 0;
    }

    // One liveness scan for the entire replan batch. The old implementation did
    // one full Primitive query per unloaded cell/domain.
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

#[cfg(test)]
fn release_cell_primitive_residency(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    coord: CellCoord,
    domain: AuthoredCellDomain,
) -> usize {
    let mut candidates = BTreeSet::new();
    take_primitive_release_candidates(world, coord, domain, &mut candidates);
    finalize_primitive_releases(world, prims, candidates)
}

fn ensure_cell_root(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    map_ref: &str,
    coord: CellCoord,
) -> EntityId {
    if let Some(existing) = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).map(|roots| roots.cell))
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
        registry.roots.insert(
            coord,
            AuthoredCellRoots {
                cell: entity,
                render: None,
                simulation: None,
            },
        );
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

fn ensure_domain_root(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    map_ref: &str,
    coord: CellCoord,
    domain: AuthoredCellDomain,
) -> EntityId {
    let cell = ensure_cell_root(world, parent, map_ref, coord);

    if let Some(existing) = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).copied())
        .and_then(|roots| match domain {
            AuthoredCellDomain::Render => roots.render,
            AuthoredCellDomain::Simulation => roots.simulation,
        })
        .filter(|entity| world.exists(*entity))
    {
        return existing;
    }

    let label = match domain {
        AuthoredCellDomain::Render => "Render",
        AuthoredCellDomain::Simulation => "Simulation",
    };
    let root = spawn_named(
        world,
        format!("World/Cells/{}/{}/{}", coord.x, coord.z, label),
    );
    let _ = world.insert(root, Transform::default());
    let _ = set_parent(world, root, Some(cell));

    if let Some(registry) = world.resource_mut::<GameReadyAuthoredMapCellRoots>() {
        if let Some(roots) = registry.roots.get_mut(&coord) {
            match domain {
                AuthoredCellDomain::Render => roots.render = Some(root),
                AuthoredCellDomain::Simulation => roots.simulation = Some(root),
            }
        }
    }
    root
}

#[inline]
pub(super) fn static_world_parent_for_prefab(
    world: &newengine_ecs::World,
    default_parent: EntityId,
    prefab: &GameReadyPrefabSpec,
) -> EntityId {
    let domain = static_world_prefab_domain(prefab);
    prefab
        .authored_cell
        .and_then(|coord| {
            world
                .resource::<GameReadyAuthoredMapCellRoots>()
                .and_then(|registry| registry.roots.get(&coord).copied())
        })
        .and_then(|roots| match domain {
            AuthoredCellDomain::Render => roots.render,
            AuthoredCellDomain::Simulation => roots.simulation,
        })
        .filter(|entity| world.exists(*entity))
        .unwrap_or(default_parent)
}

fn take_domain_root(
    world: &mut newengine_ecs::World,
    coord: CellCoord,
    domain: AuthoredCellDomain,
) -> Option<EntityId> {
    let registry = world.resource_mut::<GameReadyAuthoredMapCellRoots>()?;
    let roots = registry.roots.get_mut(&coord)?;
    match domain {
        AuthoredCellDomain::Render => roots.render.take(),
        AuthoredCellDomain::Simulation => roots.simulation.take(),
    }
}

fn remove_empty_cell_root(world: &mut newengine_ecs::World, coord: CellCoord) -> Option<EntityId> {
    let removable = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord))
        .is_some_and(|roots| roots.render.is_none() && roots.simulation.is_none());
    if !removable {
        return None;
    }
    world
        .resource_mut::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.remove(&coord))
        .map(|roots| roots.cell)
}

#[inline]
fn cell_distance(a: CellCoord, b: CellCoord) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

fn append_existing_cells(
    index: &newengine_assets_api::MapIndexV1,
    center: CellCoord,
    radius: i32,
    output: &mut BTreeSet<CellCoord>,
) {
    let radius = radius.max(0);
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let coord = CellCoord::new(center.x.saturating_add(dx), center.z.saturating_add(dz));
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
) -> Option<(CellCoord, CellCoord, f32)> {
    let center = state
        .index
        .world_to_cell([position.x, position.y, position.z])?;
    let horizontal = Vec3::new(velocity.x, 0.0, velocity.z);
    let speed = horizontal.length();
    let predicted_position = position + horizontal * state.read_ahead_sec;
    let raw_prediction = state
        .index
        .world_to_cell([
            predicted_position.x,
            predicted_position.y,
            predicted_position.z,
        ])
        .unwrap_or(center);
    let dx = (raw_prediction.x - center.x)
        .clamp(-state.max_read_ahead_cells, state.max_read_ahead_cells);
    let dz = (raw_prediction.z - center.z)
        .clamp(-state.max_read_ahead_cells, state.max_read_ahead_cells);
    Some((
        center,
        CellCoord::new(center.x.saturating_add(dx), center.z.saturating_add(dz)),
        speed,
    ))
}

fn desired_domains(
    state: &GameReadyAuthoredMapStreamingState,
    center: CellCoord,
    predicted_center: CellCoord,
) -> (BTreeSet<CellCoord>, BTreeSet<CellCoord>) {
    let mut render = BTreeSet::new();
    let mut simulation = BTreeSet::new();
    append_existing_cells(&state.index, center, state.render_radius, &mut render);
    append_existing_cells(
        &state.index,
        center,
        state.simulation_radius,
        &mut simulation,
    );

    if predicted_center != center {
        if state.render_predict_radius > 0 {
            let mut predicted = BTreeSet::new();
            append_existing_cells(
                &state.index,
                predicted_center,
                state.render_predict_radius,
                &mut predicted,
            );
            render.extend(
                predicted
                    .into_iter()
                    .filter(|coord| cell_distance(*coord, center) <= state.render_unload_radius),
            );
        }
        if state.simulation_predict_radius > 0 {
            let mut predicted = BTreeSet::new();
            append_existing_cells(
                &state.index,
                predicted_center,
                state.simulation_predict_radius,
                &mut predicted,
            );
            simulation.extend(
                predicted.into_iter().filter(|coord| {
                    cell_distance(*coord, center) <= state.simulation_unload_radius
                }),
            );
        }
    }

    (render, simulation)
}

#[inline]
fn cell_is_desired(state: &GameReadyAuthoredMapStreamingState, coord: CellCoord) -> bool {
    state.desired_render.contains(&coord) || state.desired_simulation.contains(&coord)
}

#[inline]
fn cell_needs_prepare(state: &GameReadyAuthoredMapStreamingState, coord: CellCoord) -> bool {
    (state.desired_render.contains(&coord) && !state.render_cells.contains(&coord))
        || (state.desired_simulation.contains(&coord) && !state.simulation_cells.contains(&coord))
}

fn unload_domain_collect(
    world: &mut newengine_ecs::World,
    state: &mut GameReadyAuthoredMapStreamingState,
    coord: CellCoord,
    domain: AuthoredCellDomain,
    primitive_candidates: &mut BTreeSet<PrimitiveId>,
) -> usize {
    let cancelled = cancel_static_world_cell_domain(world, &state.logical_map_ref, coord, domain);
    let removed = take_domain_root(world, coord, domain)
        .filter(|root| world.exists(*root))
        .map(|root| newengine_transform::despawn_hierarchy(world, root))
        .unwrap_or(0);
    take_primitive_release_candidates(world, coord, domain, primitive_candidates);

    match domain {
        AuthoredCellDomain::Render => {
            state.render_cells.remove(&coord);
        }
        AuthoredCellDomain::Simulation => {
            state.simulation_cells.remove(&coord);
        }
    }

    if !state.render_cells.contains(&coord) && !state.simulation_cells.contains(&coord) {
        if let Some(cell_root) =
            remove_empty_cell_root(world, coord).filter(|root| world.exists(*root))
        {
            let _ = newengine_transform::despawn_hierarchy(world, cell_root);
        }
        state.placement_ids.remove(&coord);
        state.resident_prepared.remove(&coord);
    }

    newengine_ulog_api::ulog::debug!(
        "authored map cell domain unloaded map='{}' cell={},{} domain='{}' cancelled_pending={} entities_removed={} render_resident={} simulation_resident={}",
        state.map_ref,
        coord.x,
        coord.z,
        domain.label(),
        cancelled,
        removed,
        state.render_cells.len(),
        state.simulation_cells.len(),
    );
    removed
}

fn replan_residency(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    state: &mut GameReadyAuthoredMapStreamingState,
    center: CellCoord,
    predicted_center: CellCoord,
) {
    let (desired_render, desired_simulation) = desired_domains(state, center, predicted_center);
    state.desired_render = desired_render;
    state.desired_simulation = desired_simulation;

    let stale_render = state
        .render_cells
        .iter()
        .copied()
        .filter(|coord| cell_distance(*coord, center) > state.render_unload_radius)
        .collect::<Vec<_>>();
    let stale_simulation = state
        .simulation_cells
        .iter()
        .copied()
        .filter(|coord| cell_distance(*coord, center) > state.simulation_unload_radius)
        .collect::<Vec<_>>();

    let mut primitive_candidates = BTreeSet::new();
    for coord in stale_render {
        unload_domain_collect(
            world,
            state,
            coord,
            AuthoredCellDomain::Render,
            &mut primitive_candidates,
        );
    }
    for coord in stale_simulation {
        unload_domain_collect(
            world,
            state,
            coord,
            AuthoredCellDomain::Simulation,
            &mut primitive_candidates,
        );
    }
    let released = finalize_primitive_releases(world, prims, primitive_candidates);
    if released > 0 {
        newengine_ulog_api::ulog::debug!(
            "authored map primitive eviction batch map='{}' released={}",
            state.map_ref,
            released
        );
    }

    let stale_jobs = state
        .load_jobs
        .keys()
        .copied()
        .filter(|coord| !cell_is_desired(state, *coord))
        .collect::<Vec<_>>();
    for coord in stale_jobs {
        if let Some(job) = state.load_jobs.remove(&coord) {
            let _ = job.ticket.cancel();
        }
    }

    {
        let desired_render = &state.desired_render;
        let desired_simulation = &state.desired_simulation;
        state.ready_cells.retain(|coord, _| {
            desired_render.contains(coord) || desired_simulation.contains(coord)
        });
        state.failed_cells.retain(|coord, _| {
            desired_render.contains(coord) || desired_simulation.contains(coord)
        });
        state
            .pending_cells
            .retain(|coord| desired_render.contains(coord) || desired_simulation.contains(coord));
    }
    state.pending_set = state.pending_cells.iter().copied().collect();

    // Iterate the set union directly; do not allocate a temporary desired_union BTreeSet.
    let mut candidates = state
        .desired_render
        .union(&state.desired_simulation)
        .copied()
        .filter(|coord| cell_needs_prepare(state, *coord))
        .filter(|coord| !state.resident_prepared.contains_key(coord))
        .filter(|coord| !state.pending_set.contains(coord))
        .filter(|coord| !state.load_jobs.contains_key(coord))
        .filter(|coord| !state.ready_cells.contains_key(coord))
        .collect::<Vec<_>>();

    for coord in &candidates {
        state.failed_cells.remove(coord);
    }
    candidates.sort_by_key(|coord| prepared_priority(state, *coord, center, predicted_center));
    for coord in candidates {
        if state.pending_set.insert(coord) {
            state.pending_cells.push_back(coord);
        }
    }
}

fn prepared_priority(
    state: &GameReadyAuthoredMapStreamingState,
    coord: CellCoord,
    center: CellCoord,
    predicted_center: CellCoord,
) -> (usize, i32, i32, i32, i32) {
    let simulation_needed =
        state.desired_simulation.contains(&coord) && !state.simulation_cells.contains(&coord);
    let primary_distance = cell_distance(coord, center);
    let predicted_distance = cell_distance(coord, predicted_center);
    (
        usize::from(!simulation_needed),
        primary_distance.min(predicted_distance),
        primary_distance,
        coord.x,
        coord.z,
    )
}

fn cell_load_concurrency(
    state: &GameReadyAuthoredMapStreamingState,
    thread_pool: &ThreadPoolHandle,
) -> usize {
    thread_pool
        .worker_threads()
        .saturating_sub(1)
        .max(1)
        .min(state.cell_jobs_limit)
        .min(state.max_pending_jobs)
}

fn submit_cell_jobs(
    state: &mut GameReadyAuthoredMapStreamingState,
    thread_pool: &ThreadPoolHandle,
) {
    let concurrency = cell_load_concurrency(state, thread_pool);
    let free_slots = concurrency.saturating_sub(state.load_jobs.len());
    for _ in 0..free_slots {
        let Some(coord) = state.pending_cells.pop_front() else {
            break;
        };
        state.pending_set.remove(&coord);
        if !cell_needs_prepare(state, coord)
            || state.resident_prepared.contains_key(&coord)
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
            Some(Ok(prepared)) if cell_needs_prepare(state, coord) => {
                state.failed_cells.remove(&coord);
                state.ready_cells.insert(coord, prepared);
            }
            Some(Ok(_)) => {}
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
        if !cell_needs_prepare(state, coord) || state.resident_prepared.contains_key(&coord) {
            continue;
        }
        match prepare_cell(
            &state.map_ref,
            &state.logical_map_ref,
            coord,
            &state.definition_cache,
        ) {
            Ok(prepared) => {
                state.failed_cells.remove(&coord);
                state.ready_cells.insert(coord, prepared);
            }
            Err(error) => {
                state.failed_cells.insert(coord, error);
            }
        }
    }
}

fn admit_prepared_domains(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    state: &mut GameReadyAuthoredMapStreamingState,
    coord: CellCoord,
    prepared: &PreparedMapCell,
) -> usize {
    let wants_render =
        state.desired_render.contains(&coord) && !state.render_cells.contains(&coord);
    let wants_simulation =
        state.desired_simulation.contains(&coord) && !state.simulation_cells.contains(&coord);
    let mut admitted_domains = 0usize;

    if wants_render {
        let _ = ensure_domain_root(
            world,
            state.parent,
            &state.map_ref,
            coord,
            AuthoredCellDomain::Render,
        );
        enqueue_static_world_prefabs(world, mats, state.parent, &prepared.render_prefabs);
        state.render_cells.insert(coord);
        admitted_domains = admitted_domains.saturating_add(1);
    }

    if wants_simulation {
        let _ = ensure_domain_root(
            world,
            state.parent,
            &state.map_ref,
            coord,
            AuthoredCellDomain::Simulation,
        );
        enqueue_static_world_prefabs(world, mats, state.parent, &prepared.simulation_prefabs);
        state.simulation_cells.insert(coord);
        admitted_domains = admitted_domains.saturating_add(1);
    }

    if admitted_domains > 0 {
        state
            .placement_ids
            .entry(coord)
            .or_insert_with(|| prepared.placement_ids.clone());

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
            "authored map cell domains resident map='{}' cell={},{} placements={} render_prefabs={} simulation_prefabs={} render_resident={} simulation_resident={}",
            state.map_ref,
            coord.x,
            coord.z,
            prepared.authored_placement_count,
            prepared.render_prefabs.len(),
            prepared.simulation_prefabs.len(),
            state.render_cells.len(),
            state.simulation_cells.len(),
        );
    }
    admitted_domains
}

fn next_prepared_coord(
    state: &GameReadyAuthoredMapStreamingState,
    center: CellCoord,
    predicted_center: CellCoord,
) -> Option<CellCoord> {
    state
        .resident_prepared
        .keys()
        .chain(state.ready_cells.keys())
        .copied()
        .filter(|coord| cell_needs_prepare(state, *coord))
        .min_by_key(|coord| prepared_priority(state, *coord, center, predicted_center))
}

fn admit_ready_cells(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    state: &mut GameReadyAuthoredMapStreamingState,
    center: CellCoord,
    predicted_center: CellCoord,
) -> usize {
    let mut admitted_cells = 0usize;
    for _ in 0..state.max_cells_per_tick {
        let Some(coord) = next_prepared_coord(state, center, predicted_center) else {
            break;
        };

        let prepared = if let Some(existing) = state.resident_prepared.get(&coord).cloned() {
            existing
        } else {
            let Some(ready) = state.ready_cells.remove(&coord) else {
                continue;
            };
            state.resident_prepared.insert(coord, ready.clone());
            ready
        };

        if admit_prepared_domains(world, mats, state, coord, &prepared) > 0 {
            admitted_cells = admitted_cells.saturating_add(1);
        }
    }
    admitted_cells
}

pub(in super::super) fn begin_authored_map_streaming(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    spec: Option<&GameReadyAuthoredMapStreamingSpec>,
) {
    let Some(spec) = spec else {
        return;
    };

    world.insert_resource(GameReadyAuthoredMapCellRoots::default());
    world.insert_resource(GameReadyAuthoredMapPrimitiveResidency::default());

    let render_cells = spec
        .initial_render_cells
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let simulation_cells = spec
        .initial_simulation_cells
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    for coord in spec.initial_render_cells.iter().copied() {
        let _ = ensure_domain_root(
            world,
            parent,
            &spec.map_ref,
            coord,
            AuthoredCellDomain::Render,
        );
    }
    for coord in spec.initial_simulation_cells.iter().copied() {
        let _ = ensure_domain_root(
            world,
            parent,
            &spec.map_ref,
            coord,
            AuthoredCellDomain::Simulation,
        );
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
    let read_ahead_sec =
        crate::env_config::var_f32("NEWENGINE_AUTHORED_MAP_READ_AHEAD_SEC", 0.75, 0.0, 3.0);
    let default_max_read_ahead_cells = spec
        .render_radius
        .max(spec.simulation_radius)
        .saturating_add(1)
        .clamp(1, 4);
    let max_read_ahead_cells = crate::env_config::var_i32(
        "NEWENGINE_AUTHORED_MAP_MAX_READ_AHEAD_CELLS",
        default_max_read_ahead_cells,
        0,
        8,
    );
    let render_predict_radius = crate::env_config::var_i32(
        "NEWENGINE_AUTHORED_MAP_PREDICT_RADIUS",
        1.min(spec.render_radius),
        0,
        spec.render_radius.max(0),
    );
    let simulation_predict_radius = crate::env_config::var_i32(
        "NEWENGINE_AUTHORED_MAP_SIMULATION_PREDICT_RADIUS",
        1.min(spec.simulation_radius),
        0,
        spec.simulation_radius.max(0),
    );
    let cell_jobs_limit = crate::env_config::var_usize(
        "NEWENGINE_AUTHORED_MAP_CELL_JOBS",
        spec.max_cells_per_tick.max(1).min(4),
        1,
        8,
    );

    world.insert_resource(GameReadyAuthoredMapStreamingState {
        parent,
        map_ref: spec.map_ref.clone(),
        logical_map_ref,
        index: spec.index.clone(),
        render_radius: spec.render_radius,
        simulation_radius: spec.simulation_radius,
        render_unload_radius: spec.render_unload_radius,
        simulation_unload_radius: spec.simulation_unload_radius,
        max_cells_per_tick: spec.max_cells_per_tick,
        max_pending_jobs,
        read_ahead_sec,
        max_read_ahead_cells,
        render_predict_radius,
        simulation_predict_radius,
        cell_jobs_limit,
        render_cells: render_cells.clone(),
        simulation_cells: simulation_cells.clone(),
        desired_render: render_cells,
        desired_simulation: simulation_cells,
        resident_prepared: BTreeMap::new(),
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
        "authored map streaming initialized map='{}' cells_total={} render_resident={} simulation_resident={} render_radius={} simulation_radius={} render_unload_radius={} simulation_unload_radius={} max_cells_per_tick={} max_pending_jobs={} policy='YMAP index resident; async cell prepare; dual-domain residency; cached runtime tuning'",
        spec.map_ref,
        spec.index.cells.len(),
        spec.initial_render_cells.len(),
        spec.initial_simulation_cells.len(),
        spec.render_radius,
        spec.simulation_radius,
        spec.render_unload_radius,
        spec.simulation_unload_radius,
        spec.max_cells_per_tick,
        max_pending_jobs,
    );
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
    let Some((center, predicted_center, speed)) =
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
            "authored map streaming tick map='{}' center={},{} predicted={},{} speed_mps={:.2} read_ahead_sec={:.2} max_read_ahead_cells={} admitted_cells={} render_resident={} simulation_resident={} queued={} jobs={} ready={} prepared={} failed={}",
            state.map_ref,
            center.x,
            center.z,
            predicted_center.x,
            predicted_center.z,
            speed,
            state.read_ahead_sec,
            state.max_read_ahead_cells,
            admitted,
            state.render_cells.len(),
            state.simulation_cells.len(),
            state.pending_cells.len(),
            state.load_jobs.len(),
            state.ready_cells.len(),
            state.resident_prepared.len(),
            state.failed_cells.len(),
        );
    }

    world.insert_resource(state);
}

#[cfg(test)]
#[path = "authored_map_streaming/tests.rs"]
mod tests;
