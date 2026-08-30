use super::super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::super::*;
use super::materials::{register_forest_road_materials, ForestRoadMaterials};
use super::spawn::{
    spawn_box_collision_ydd_prefab_from_decoded, spawn_collision_ydd_prefab_from_decoded,
    spawn_dynamic_ydd_prefab_from_decoded, spawn_static_ydd_prefab_from_decoded,
};
use super::{
    StaticWorldSpawnSummary, BOX_COLLISION_WORLD_PROXY, COLLISION_WORLD_PROXY, DYNAMIC_WORLD_PROXY,
    STATIC_WORLD_PROXY,
};
use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use newengine_engine_runtime::gameplay::WorldAssemblyProgress;
use parking_lot::Mutex;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

type StaticWorldDecodeResult = Arc<Mutex<Option<Result<Vec<DecodedPrefabMeshPart>, String>>>>;

struct StaticWorldDecodeJob {
    ticket: TaskTicket,
    result: StaticWorldDecodeResult,
}

#[derive(Default)]
struct StaticWorldSourceQueue {
    collision: VecDeque<GameReadyPrefabSpec>,
    visual: VecDeque<GameReadyPrefabSpec>,
}

struct GameReadyStaticWorldStreamingState {
    parent: EntityId,
    /// Placements are grouped by normalized YDD source so readiness never scans the full world.
    pending_by_source: BTreeMap<String, StaticWorldSourceQueue>,
    /// Unique decode work in collision-first deterministic source order.
    decode_backlog: VecDeque<String>,
    /// Terminal sources ready for bounded ECS admission. Collision always wins over visual.
    ready_collision_sources: BTreeSet<String>,
    ready_visual_sources: BTreeSet<String>,
    pending_count: usize,
    materials: ForestRoadMaterials,
    decoded_cache: BTreeMap<String, Arc<Vec<DecodedPrefabMeshPart>>>,
    decode_jobs: BTreeMap<String, StaticWorldDecodeJob>,
    decode_errors: BTreeMap<String, String>,
    summary: StaticWorldSpawnSummary,
    started_at: Instant,
}

pub(in super::super) fn begin_static_world_prefabs(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefabs: &[GameReadyPrefabSpec],
) -> StaticWorldSpawnSummary {
    let mut candidates = prefabs
        .iter()
        .filter(|prefab| {
            prefab.enabled
                && (prefab.proxy.trim().eq_ignore_ascii_case(STATIC_WORLD_PROXY)
                    || prefab
                        .proxy
                        .trim()
                        .eq_ignore_ascii_case(DYNAMIC_WORLD_PROXY)
                    || is_collision_proxy(prefab.proxy.trim()))
        })
        .cloned()
        .collect::<Vec<_>>();
    // Canonicalize the source path once. The old path normalized it repeatedly inside
    // queue scans, ready checks and release checks, creating transient Strings proportional
    // to the number of pending placements.
    for prefab in &mut candidates {
        prefab.source = prefab.source.trim().replace('\\', "/");
    }

    // Collision is launch-critical and is admitted before render-only static geometry.
    // Decoded source packets remain cached for the later visual declaration.
    candidates.sort_by(|a, b| {
        let a_collision = is_simulation_proxy(a.proxy.trim());
        let b_collision = is_simulation_proxy(b.proxy.trim());
        // Collision is launch-critical: admit it before render-only static geometry.
        // Within the same role retain deterministic source order for cache locality.
        b_collision
            .cmp(&a_collision)
            .then_with(|| a.source.cmp(&b.source))
    });

    let total = candidates.len() as u32;
    let pending_count = candidates.len();
    let mut pending_by_source = BTreeMap::<String, StaticWorldSourceQueue>::new();
    let mut decode_backlog = VecDeque::<String>::new();
    let mut seen_sources = BTreeSet::<String>::new();
    for prefab in candidates {
        let source = prefab.source.clone();
        if seen_sources.insert(source.clone()) {
            decode_backlog.push_back(source.clone());
        }
        let queue = pending_by_source.entry(source).or_default();
        if is_simulation_proxy(prefab.proxy.trim()) {
            queue.collision.push_back(prefab);
        } else {
            queue.visual.push_back(prefab);
        }
    }

    world.insert_resource(WorldAssemblyProgress {
        total,
        pending: total,
        ..WorldAssemblyProgress::default()
    });
    if total == 0 {
        return StaticWorldSpawnSummary::default();
    }

    world.insert_resource(GameReadyStaticWorldStreamingState {
        parent,
        pending_by_source,
        decode_backlog,
        ready_collision_sources: BTreeSet::new(),
        ready_visual_sources: BTreeSet::new(),
        pending_count,
        materials: register_forest_road_materials(mats),
        decoded_cache: BTreeMap::new(),
        decode_jobs: BTreeMap::new(),
        decode_errors: BTreeMap::new(),
        summary: StaticWorldSpawnSummary::default(),
        started_at: Instant::now(),
    });
    newengine_ulog_api::ulog::info!(
        "static world bootstrap queued models={} policy='parallel YDD decode on engine.threading; bounded ECS/GPU admission'",
        total
    );
    StaticWorldSpawnSummary {
        models: total,
        ..StaticWorldSpawnSummary::default()
    }
}

pub(super) fn enqueue_static_world_prefabs(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefabs: &[GameReadyPrefabSpec],
) {
    if prefabs.is_empty() {
        return;
    }
    let Some(mut state) = world.remove_resource::<GameReadyStaticWorldStreamingState>() else {
        let _ = begin_static_world_prefabs(world, mats, parent, prefabs);
        return;
    };

    let mut candidates = prefabs
        .iter()
        .filter(|prefab| {
            prefab.enabled
                && (prefab.proxy.trim().eq_ignore_ascii_case(STATIC_WORLD_PROXY)
                    || prefab
                        .proxy
                        .trim()
                        .eq_ignore_ascii_case(DYNAMIC_WORLD_PROXY)
                    || is_collision_proxy(prefab.proxy.trim()))
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        is_simulation_proxy(b.proxy.trim())
            .cmp(&is_simulation_proxy(a.proxy.trim()))
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.id.cmp(&b.id))
    });

    let added = candidates.len();
    for mut prefab in candidates {
        prefab.source = prefab.source.trim().replace('\\', "/");
        let source = prefab.source.clone();
        let source_known = state.pending_by_source.contains_key(&source)
            || state.decode_jobs.contains_key(&source)
            || state.decoded_cache.contains_key(&source)
            || state.decode_errors.contains_key(&source)
            || state.decode_backlog.iter().any(|queued| queued == &source);
        if !source_known {
            state.decode_backlog.push_back(source.clone());
        }
        let queue = state.pending_by_source.entry(source).or_default();
        if is_collision_proxy(prefab.proxy.trim()) {
            queue.collision.push_back(prefab);
        } else {
            queue.visual.push_back(prefab);
        }
    }
    state.pending_count = state.pending_count.saturating_add(added);
    if let Some(progress) = world.resource_mut::<WorldAssemblyProgress>() {
        progress.total = progress
            .total
            .saturating_add(added.min(u32::MAX as usize) as u32);
        progress.pending = progress
            .pending
            .saturating_add(added.min(u32::MAX as usize) as u32);
    }
    newengine_ulog_api::ulog::debug!(
        "static world streaming enqueue placements={} pending={} sources={}",
        added,
        state.pending_count,
        state.pending_by_source.len(),
    );
    world.insert_resource(state);
}

pub(super) fn cancel_static_world_cell_domain(
    world: &mut newengine_ecs::World,
    map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
    domain: super::authored_map_streaming::AuthoredCellDomain,
) -> usize {
    let Some(mut state) = world.remove_resource::<GameReadyStaticWorldStreamingState>() else {
        return 0;
    };
    let mut removed = 0usize;
    let sources = state.pending_by_source.keys().cloned().collect::<Vec<_>>();
    for source in sources {
        let Some(queue) = state.pending_by_source.get_mut(&source) else {
            continue;
        };
        let before = queue.collision.len().saturating_add(queue.visual.len());
        let keep = |prefab: &GameReadyPrefabSpec| {
            !(prefab.authored_map_ref == map_ref
                && prefab.authored_cell == Some(coord)
                && super::authored_map_streaming::static_world_prefab_domain(prefab) == domain)
        };
        queue.collision.retain(&keep);
        queue.visual.retain(keep);
        let after = queue.collision.len().saturating_add(queue.visual.len());
        removed = removed.saturating_add(before.saturating_sub(after));
        if queue.collision.is_empty() && queue.visual.is_empty() {
            state.pending_by_source.remove(&source);
            state.ready_collision_sources.remove(&source);
            state.ready_visual_sources.remove(&source);
            state.decode_backlog.retain(|queued| queued != &source);
            if let Some(job) = state.decode_jobs.remove(&source) {
                let _ = job.ticket.cancel();
            }
            release_static_world_source_packet(&mut state, &source);
        }
    }
    state.pending_count = state.pending_count.saturating_sub(removed);
    if let Some(progress) = world.resource_mut::<WorldAssemblyProgress>() {
        progress.pending = progress
            .pending
            .saturating_sub(removed.min(u32::MAX as usize) as u32);
    }
    world.insert_resource(state);
    removed
}

#[inline]
fn is_collision_proxy(proxy: &str) -> bool {
    proxy.eq_ignore_ascii_case(COLLISION_WORLD_PROXY)
        || proxy.eq_ignore_ascii_case(BOX_COLLISION_WORLD_PROXY)
}

#[inline]
fn is_simulation_proxy(proxy: &str) -> bool {
    is_collision_proxy(proxy) || proxy.eq_ignore_ascii_case(DYNAMIC_WORLD_PROXY)
}

fn mark_static_world_source_terminal(state: &mut GameReadyStaticWorldStreamingState, source: &str) {
    let Some(queue) = state.pending_by_source.get(source) else {
        return;
    };
    if !queue.collision.is_empty() {
        state.ready_collision_sources.insert(source.to_owned());
    } else if !queue.visual.is_empty() {
        state.ready_visual_sources.insert(source.to_owned());
    }
}

fn pop_static_world_admittable(
    state: &mut GameReadyStaticWorldStreamingState,
) -> Option<(String, GameReadyPrefabSpec, bool)> {
    let (source, collision_tier) = if let Some(source) = state.ready_collision_sources.first() {
        (source.clone(), true)
    } else {
        (state.ready_visual_sources.first()?.clone(), false)
    };

    let queue = state.pending_by_source.get_mut(&source)?;
    let prefab = if collision_tier {
        queue.collision.pop_front()
    } else {
        queue.visual.pop_front()
    }?;
    state.pending_count = state.pending_count.saturating_sub(1);

    if collision_tier && queue.collision.is_empty() {
        state.ready_collision_sources.remove(&source);
        if !queue.visual.is_empty() {
            state.ready_visual_sources.insert(source.clone());
        }
    } else if !collision_tier && queue.visual.is_empty() {
        state.ready_visual_sources.remove(&source);
    }

    let source_finished = queue.collision.is_empty() && queue.visual.is_empty();
    if source_finished {
        state.pending_by_source.remove(&source);
        state.ready_collision_sources.remove(&source);
        state.ready_visual_sources.remove(&source);
    }
    Some((source, prefab, source_finished))
}

#[inline]
fn release_static_world_source_packet(
    state: &mut GameReadyStaticWorldStreamingState,
    source: &str,
) {
    state.decoded_cache.remove(source);
    state.decode_errors.remove(source);
}

fn static_world_decode_concurrency(thread_pool: &ThreadPoolHandle) -> usize {
    let available_workers = thread_pool.worker_threads();
    // AssetManager serializes portions of its dictionary cache, so unbounded
    // concurrency only creates contention. Scale modestly with the worker pool
    // while preserving the historical three-job baseline on larger machines.
    let adaptive_default = available_workers.saturating_sub(1).clamp(1, 3) as u32;
    crate::env_config::var_u32("NEWENGINE_STATIC_WORLD_DECODE_JOBS", adaptive_default, 1, 6)
        as usize
}

fn static_world_admission_budget_ms() -> f32 {
    crate::env_config::var_f32("NEWENGINE_STATIC_WORLD_BOOTSTRAP_BUDGET_MS", 3.5, 0.5, 16.0)
}

fn submit_static_world_decode_jobs(
    state: &mut GameReadyStaticWorldStreamingState,
    thread_pool: &ThreadPoolHandle,
) {
    let max_jobs = static_world_decode_concurrency(thread_pool);
    let free_slots = max_jobs.saturating_sub(state.decode_jobs.len());
    if free_slots == 0 {
        return;
    }

    let mut sources = Vec::with_capacity(free_slots);
    for _ in 0..free_slots {
        let Some(source) = state.decode_backlog.pop_front() else {
            break;
        };
        sources.push(source);
    }

    for source in sources {
        let worker_source = source.clone();
        let result = Arc::new(Mutex::new(None));
        let result_out = Arc::clone(&result);
        let request = TaskRequest::new("static.world.ydd.decode")
            .with_source("scene.bridge.game-ready")
            .with_owner("engine.scene")
            .with_category("asset-decode")
            .with_lane(TaskLane::AssetIo)
            .with_priority(TaskPriority::Interactive)
            .with_task_id(format!(
                "scene.static-world.decode.{:016x}",
                newengine_primitives::fnv1a_64(&source)
            ));
        let host_context = newengine_plugin_host::current_host_context();
        let ticket = thread_pool.submit_request(request, move || {
            let decoded = newengine_plugin_host::with_host_context(&host_context, || {
                decode_runtime_ydd_prefab(&worker_source)
            });
            *result_out.lock() = Some(decoded);
        });
        state
            .decode_jobs
            .insert(source, StaticWorldDecodeJob { ticket, result });
    }
}

fn poll_static_world_decode_jobs(state: &mut GameReadyStaticWorldStreamingState) {
    let ready = state
        .decode_jobs
        .iter()
        .filter(|(_, job)| job.ticket.is_complete())
        .map(|(source, _)| source.clone())
        .collect::<Vec<_>>();
    for source in ready {
        let Some(job) = state.decode_jobs.remove(&source) else {
            continue;
        };
        let result = job.result.lock().take();
        match result {
            Some(Ok(decoded)) => {
                state
                    .decoded_cache
                    .insert(source.clone(), Arc::new(decoded));
            }
            Some(Err(error)) => {
                state.decode_errors.insert(source.clone(), error);
            }
            None => {
                state.decode_errors.insert(
                    source.clone(),
                    "static world decode task completed without result".to_owned(),
                );
            }
        }
        mark_static_world_source_terminal(state, &source);
    }
}

fn decode_one_static_world_source_synchronously(state: &mut GameReadyStaticWorldStreamingState) {
    let Some(source) = state.decode_backlog.pop_front() else {
        return;
    };
    match decode_runtime_ydd_prefab(&source) {
        Ok(decoded) => {
            state
                .decoded_cache
                .insert(source.clone(), Arc::new(decoded));
        }
        Err(error) => {
            state.decode_errors.insert(source.clone(), error);
        }
    }
    mark_static_world_source_terminal(state, &source);
}

pub(crate) fn tick_game_ready_static_world_prefabs(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    let Some(mut state) = world.remove_resource::<GameReadyStaticWorldStreamingState>() else {
        return;
    };
    if let Some(thread_pool) = thread_pool {
        submit_static_world_decode_jobs(&mut state, thread_pool);
        poll_static_world_decode_jobs(&mut state);
    } else {
        decode_one_static_world_source_synchronously(&mut state);
    }

    let max_models = crate::env_config::var_u32(
        "NEWENGINE_STATIC_WORLD_BOOTSTRAP_MODELS_PER_FRAME",
        8,
        1,
        32,
    ) as usize;
    let admission_budget_ms = static_world_admission_budget_ms();
    let admission_started = Instant::now();

    let mut completed_this_frame = 0u32;
    let mut failed_this_frame = 0u32;
    for _ in 0..max_models {
        let admitted = completed_this_frame.saturating_add(failed_this_frame);
        if admitted > 0 && admission_started.elapsed().as_secs_f32() * 1000.0 >= admission_budget_ms
        {
            break;
        }
        let Some((source, prefab, source_finished)) = pop_static_world_admittable(&mut state)
        else {
            break;
        };
        if let Some(error) = state.decode_errors.get(source.as_str()).cloned() {
            failed_this_frame = failed_this_frame.saturating_add(1);
            newengine_ulog_api::ulog::error!(
                "static world prefab decode failed id='{}' source='{}' err='{}'",
                prefab.id,
                prefab.source,
                error,
            );
            if source_finished {
                release_static_world_source_packet(&mut state, &source);
            }
            continue;
        }
        let Some(decoded) = state.decoded_cache.get(source.as_str()).cloned() else {
            failed_this_frame = failed_this_frame.saturating_add(1);
            newengine_ulog_api::ulog::error!(
                "static world prefab admission lost decoded source id='{}' source='{}'",
                prefab.id,
                prefab.source,
            );
            if source_finished {
                release_static_world_source_packet(&mut state, &source);
            }
            continue;
        };
        let prefab_parent = super::authored_map_streaming::static_world_parent_for_prefab(
            world,
            state.parent,
            &prefab,
        );

        let result = if prefab
            .proxy
            .trim()
            .eq_ignore_ascii_case(BOX_COLLISION_WORLD_PROXY)
        {
            spawn_box_collision_ydd_prefab_from_decoded(
                world,
                prefab_parent,
                &prefab,
                decoded.as_slice(),
            )
        } else if prefab
            .proxy
            .trim()
            .eq_ignore_ascii_case(COLLISION_WORLD_PROXY)
        {
            spawn_collision_ydd_prefab_from_decoded(
                world,
                prefab_parent,
                &prefab,
                decoded.as_slice(),
            )
        } else if prefab
            .proxy
            .trim()
            .eq_ignore_ascii_case(DYNAMIC_WORLD_PROXY)
        {
            spawn_dynamic_ydd_prefab_from_decoded(
                world,
                prims,
                mats,
                prefab_parent,
                &prefab,
                state.materials,
                decoded.as_slice(),
            )
        } else {
            spawn_static_ydd_prefab_from_decoded(
                world,
                prims,
                mats,
                prefab_parent,
                &prefab,
                state.materials,
                decoded.as_slice(),
            )
        };
        match result {
            Ok((parts, triangles)) => {
                if !is_collision_proxy(prefab.proxy.trim()) {
                    super::authored_map_streaming::record_static_world_primitive_residency(
                        world,
                        &prefab,
                        decoded.as_slice(),
                    );
                }
                state.summary.models = state.summary.models.saturating_add(1);
                state.summary.parts = state.summary.parts.saturating_add(parts);
                state.summary.triangles = state.summary.triangles.saturating_add(triangles);
                completed_this_frame = completed_this_frame.saturating_add(1);
                newengine_ulog_api::ulog::debug!(
                    "static world prefab streamed id='{}' source='{}' material='{}' position={:?} parts={} triangles={} pending={} decode_jobs={} decoded_ready={}",
                    prefab.id,
                    prefab.source,
                    prefab.material,
                    prefab.position,
                    parts,
                    triangles,
                    state.pending_count,
                    state.decode_jobs.len(),
                    state.decoded_cache.len(),
                );
            }
            Err(error) => {
                failed_this_frame = failed_this_frame.saturating_add(1);
                newengine_ulog_api::ulog::error!(
                    "static world prefab failed id='{}' source='{}' err='{}'",
                    prefab.id,
                    prefab.source,
                    error,
                );
            }
        }
        if source_finished {
            release_static_world_source_packet(&mut state, &source);
        }
    }

    let pending = state.pending_count.min(u32::MAX as usize) as u32;
    if let Some(residency) = world.resource_mut::<WorldAssemblyProgress>() {
        residency.completed = residency.completed.saturating_add(completed_this_frame);
        residency.failed = residency.failed.saturating_add(failed_this_frame);
        residency.pending = pending;
        residency.parts = state.summary.parts;
        residency.triangles = state.summary.triangles;
    }

    if pending == 0 {
        let elapsed_ms = state.started_at.elapsed().as_secs_f32() * 1000.0;
        newengine_ulog_api::ulog::info!(
            "static world bootstrap completed models={} parts={} triangles={} failed={} elapsed_ms={:.2} policy='incremental; no event-loop starvation'",
            state.summary.models,
            state.summary.parts,
            state.summary.triangles,
            world
                .resource::<WorldAssemblyProgress>()
                .map(WorldAssemblyProgress::failed)
                .unwrap_or(0),
            elapsed_ms,
        );
        let _ = validate_scene_objects(world, "game-ready.static-world-complete");
    } else {
        world.insert_resource(state);
    }
}
