use super::super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::super::*;
use super::materials::{register_forest_road_materials, ForestRoadMaterials};
use super::spawn::{
    spawn_collision_ydd_prefab_from_decoded, spawn_dynamic_ydd_prefab_from_decoded,
    spawn_static_ydd_prefab_from_decoded,
};
use super::{
    StaticWorldSpawnSummary, COLLISION_WORLD_PROXY, DYNAMIC_WORLD_PROXY, STATIC_WORLD_PROXY,
};
use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use newengine_engine_runtime::gameplay::WorldAssemblyProgress;
use parking_lot::Mutex;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

type StaticWorldDecodeResult = Arc<Mutex<Option<Result<Vec<DecodedPrefabMeshPart>, String>>>>;

struct StaticWorldDecodeJob {
    ticket: TaskTicket,
    result: StaticWorldDecodeResult,
}

struct GameReadyStaticWorldStreamingState {
    parent: EntityId,
    pending: VecDeque<GameReadyPrefabSpec>,
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
                    || prefab
                        .proxy
                        .trim()
                        .eq_ignore_ascii_case(COLLISION_WORLD_PROXY))
        })
        .cloned()
        .collect::<Vec<_>>();
    // Collision is launch-critical and is admitted before render-only static geometry.
    // Decoded source packets remain cached for the later visual declaration.
    candidates.sort_by(|a, b| {
        let a_collision = a.proxy.trim().eq_ignore_ascii_case(COLLISION_WORLD_PROXY);
        let b_collision = b.proxy.trim().eq_ignore_ascii_case(COLLISION_WORLD_PROXY);
        let a_source = a.source.trim().replace('\\', "/");
        let b_source = b.source.trim().replace('\\', "/");
        // Collision is launch-critical: admit it before render-only static geometry.
        // Within the same role retain deterministic source order for cache locality.
        b_collision
            .cmp(&a_collision)
            .then_with(|| a_source.cmp(&b_source))
    });

    let total = candidates.len() as u32;
    world.insert_resource(WorldAssemblyProgress {
        total,
        pending: total,
        ..WorldAssemblyProgress::default()
    });
    if candidates.is_empty() {
        return StaticWorldSpawnSummary::default();
    }

    world.insert_resource(GameReadyStaticWorldStreamingState {
        parent,
        pending: candidates.into(),
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

fn static_world_source(prefab: &GameReadyPrefabSpec) -> String {
    prefab.source.trim().replace('\\', "/")
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

    let mut sources = Vec::<String>::new();
    for prefab in &state.pending {
        let source = static_world_source(prefab);
        if state.decoded_cache.contains_key(&source)
            || state.decode_jobs.contains_key(&source)
            || state.decode_errors.contains_key(&source)
            || sources.contains(&source)
        {
            continue;
        }
        sources.push(source);
        if sources.len() >= free_slots {
            break;
        }
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
        let ticket = thread_pool.submit_request(request, move || {
            *result_out.lock() = Some(decode_runtime_ydd_prefab(&worker_source));
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
                state.decoded_cache.insert(source, Arc::new(decoded));
            }
            Some(Err(error)) => {
                state.decode_errors.insert(source, error);
            }
            None => {
                state.decode_errors.insert(
                    source,
                    "static world decode task completed without result".to_owned(),
                );
            }
        }
    }
}

fn decode_one_static_world_source_synchronously(state: &mut GameReadyStaticWorldStreamingState) {
    let Some(source) = state.pending.iter().find_map(|prefab| {
        let source = static_world_source(prefab);
        (!state.decoded_cache.contains_key(&source) && !state.decode_errors.contains_key(&source))
            .then_some(source)
    }) else {
        return;
    };
    match decode_runtime_ydd_prefab(&source) {
        Ok(decoded) => {
            state.decoded_cache.insert(source, Arc::new(decoded));
        }
        Err(error) => {
            state.decode_errors.insert(source, error);
        }
    }
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
        if let Some(failed_position) = state.pending.iter().position(|prefab| {
            state
                .decode_errors
                .contains_key(&static_world_source(prefab))
        }) {
            let Some(prefab) = state.pending.remove(failed_position) else {
                continue;
            };
            let source = static_world_source(&prefab);
            let error = state
                .decode_errors
                .get(&source)
                .cloned()
                .unwrap_or_else(|| "unknown static world decode failure".to_owned());
            failed_this_frame = failed_this_frame.saturating_add(1);
            newengine_ulog_api::ulog::error!(
                "static world prefab decode failed id='{}' source='{}' err='{}'",
                prefab.id,
                prefab.source,
                error,
            );
            continue;
        }

        let Some(ready_position) = state.pending.iter().position(|prefab| {
            state
                .decoded_cache
                .contains_key(&static_world_source(prefab))
        }) else {
            break;
        };
        let Some(prefab) = state.pending.remove(ready_position) else {
            continue;
        };
        let source = static_world_source(&prefab);
        let Some(decoded) = state.decoded_cache.get(&source).cloned() else {
            continue;
        };

        let result = if prefab
            .proxy
            .trim()
            .eq_ignore_ascii_case(COLLISION_WORLD_PROXY)
        {
            spawn_collision_ydd_prefab_from_decoded(
                world,
                state.parent,
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
                state.parent,
                &prefab,
                state.materials,
                decoded.as_slice(),
            )
        } else {
            spawn_static_ydd_prefab_from_decoded(
                world,
                prims,
                mats,
                state.parent,
                &prefab,
                state.materials,
                decoded.as_slice(),
            )
        };
        match result {
            Ok((parts, triangles)) => {
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
                    state.pending.len(),
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
    }

    let pending = state.pending.len() as u32;
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
