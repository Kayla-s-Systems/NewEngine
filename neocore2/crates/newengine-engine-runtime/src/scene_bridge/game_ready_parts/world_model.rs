use super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::*;
use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use newengine_materials::api::MaterialRegistryApi;
use parking_lot::Mutex;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

const WORLD_MATERIAL_LIBRARY: &str = "materials/world_game_ready.nemat";
const STATIC_WORLD_PROXY: &str = "world_static_ydd";
const COLLISION_WORLD_PROXY: &str = "world_collision_ydd";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StaticWorldSpawnSummary {
    pub models: u32,
    pub parts: u32,
    pub triangles: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GameReadyStaticWorldResidency {
    total: u32,
    completed: u32,
    failed: u32,
    pending: u32,
    parts: u32,
    triangles: u64,
}

impl GameReadyStaticWorldResidency {
    #[inline]
    pub(crate) fn is_ready(&self) -> bool {
        self.pending == 0
    }

    #[inline]
    pub(crate) fn total(&self) -> u32 {
        self.total
    }

    #[inline]
    pub(crate) fn completed(&self) -> u32 {
        self.completed
    }

    #[inline]
    pub(crate) fn failed(&self) -> u32 {
        self.failed
    }

    #[inline]
    pub(crate) fn pending(&self) -> u32 {
        self.pending
    }
}

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

#[derive(Clone, Copy, Debug)]
struct ForestRoadMaterials {
    road: MaterialId,
    terrain: MaterialId,
    props: MaterialId,
}

fn material_spec(entry: &str, roughness: f32) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: Some(format!("{WORLD_MATERIAL_LIBRARY}@{entry}")),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness,
        normal_scale: 0.0,
        occlusion_strength: 1.0,
    }
}

fn register_forest_road_materials(mats: &MaterialRegistry) -> ForestRoadMaterials {
    let opaque_world = MaterialFlags::DOUBLE_SIDED
        .union(MaterialFlags::CAST_SHADOWS)
        .union(MaterialFlags::RECEIVE_SHADOWS);
    let road = material_spec("forest_road_road", 0.92);
    let terrain = material_spec("forest_road_terrain", 0.96);
    let props = material_spec("forest_road_props", 0.82);

    ForestRoadMaterials {
        road: register_material(
            mats,
            "World/ForestRoad/Road",
            [0.24, 0.16, 0.08, 1.0],
            [0.0, 0.0, 0.0],
            1.0,
            opaque_world,
            &road,
        ),
        terrain: register_material(
            mats,
            "World/ForestRoad/Terrain",
            [0.10, 0.18, 0.08, 1.0],
            [0.0, 0.0, 0.0],
            1.0,
            opaque_world,
            &terrain,
        ),
        props: register_material(
            mats,
            "World/ForestRoad/Props",
            [0.24, 0.13, 0.055, 1.0],
            [0.0, 0.0, 0.0],
            1.0,
            opaque_world,
            &props,
        ),
    }
}

fn register_authored_prefab_material(
    mats: &MaterialRegistry,
    prefab: &GameReadyPrefabSpec,
) -> Option<MaterialId> {
    let asset = prefab.material.trim().replace('\\', "/");
    if asset.is_empty() {
        return None;
    }
    if !asset.to_ascii_lowercase().contains(".nemat@") {
        newengine_ulog_api::ulog::warn!(
            "static world prefab id='{}' material='{}' ignored: expected .nemat@entry",
            prefab.id,
            asset,
        );
        return None;
    }
    let spec = GameReadyMaterialSpec {
        asset: Some(asset.clone()),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.82,
        normal_scale: 0.72,
        occlusion_strength: 1.0,
    };
    // Authored `.nemat` owns the double-sided policy. Runtime contributes only
    // the static-world shadow requirements so terrain can explicitly cull its
    // underside while foliage/props remain two-sided when their asset says so.
    let flags = MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS);
    Some(register_material(
        mats,
        &format!("World/Static/{}/Material", prefab.id),
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
        1.0,
        flags,
        &spec,
    ))
}

#[inline]
fn static_world_receive_only_shadow_slot(slot: &str) -> bool {
    let slot = slot.trim().to_ascii_lowercase();
    [
        "dirt_road_bare",
        "ground_dirt",
        "terrain_far",
        "aerial_grass",
        "grass_close",
        "cobblestone",
    ]
    .iter()
    .any(|tag| slot.contains(tag))
}

#[inline]
fn material_for_slot(materials: ForestRoadMaterials, slot: &str) -> MaterialId {
    let slot = slot.trim().to_ascii_lowercase();
    if slot.contains("terrain") || slot.contains("ground") {
        materials.terrain
    } else if slot.contains("props") || slot.contains("wood") || slot.contains("rock") {
        materials.props
    } else {
        materials.road
    }
}

fn spawn_collision_ydd_prefab_from_decoded(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    prefab: &GameReadyPrefabSpec,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(u32, u64), String> {
    let source = prefab.source.trim().replace('\\', "/");
    if !source.to_ascii_lowercase().contains(".ydd@") {
        return Err(format!(
            "collision world prefab id='{}' source='{}' rejected: expected .ydd@entry",
            prefab.id, source
        ));
    }
    let mut vertices = Vec::<[f32; 3]>::new();
    let mut triangles = Vec::<[u32; 3]>::new();
    let mut part_count = 0u32;
    for part in decoded {
        let base = u32::try_from(vertices.len())
            .map_err(|_| "collision mesh exceeds u32 vertex addressing".to_owned())?;
        for vertex in &part.mesh.vertices {
            vertices.push([
                vertex.pos[0] * prefab.scale.x,
                vertex.pos[1] * prefab.scale.y,
                vertex.pos[2] * prefab.scale.z,
            ]);
        }
        for triangle in part.mesh.indices.chunks_exact(3) {
            triangles.push([
                base.checked_add(triangle[0])
                    .ok_or("collision index overflow")?,
                base.checked_add(triangle[1])
                    .ok_or("collision index overflow")?,
                base.checked_add(triangle[2])
                    .ok_or("collision index overflow")?,
            ]);
        }
        part_count = part_count.saturating_add(1);
    }
    let triangle_count = triangles.len() as u64;
    let collider =
        crate::gameplay::StaticMeshCollider::new(vertices, triangles)?.with_material(0.94, 0.0);
    let local_bounds = collider.local_bounds;
    let vertex_count = collider.vertices.len();
    let entity = spawn_named(world, format!("World/Collision/{}", prefab.id));
    let _ = set_parent(world, entity, Some(parent));
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        prefab.rotation_ypr.x,
        prefab.rotation_ypr.y,
        prefab.rotation_ypr.z,
    );
    let _ = world.insert(
        entity,
        Transform {
            position: prefab.position,
            rotation,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(entity, Bounds::from_local_aabb(local_bounds));
    let _ = world.insert(entity, collider);
    let _ = world.insert(
        entity,
        crate::gameplay::PhysicsSurface {
            id: "surface.dirt_road".to_owned(),
            footstep_event: "audio.footstep.dirt".to_owned(),
            landing_event: "audio.landing.dirt".to_owned(),
        },
    );
    newengine_ulog_api::ulog::debug!(
        "static world collision spawned id='{}' source='{}' entity={:?} parts={} vertices={} triangles={} position={:?} rotation_ypr={:?} scale_baked={:?} bounds_min={:?} bounds_max={:?}",
        prefab.id,
        prefab.source,
        entity,
        part_count,
        vertex_count,
        triangle_count,
        prefab.position,
        prefab.rotation_ypr,
        prefab.scale,
        local_bounds.min,
        local_bounds.max,
    );
    Ok((part_count, triangle_count))
}

fn spawn_static_ydd_prefab_from_decoded(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefab: &GameReadyPrefabSpec,
    materials: ForestRoadMaterials,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(u32, u64), String> {
    let source = prefab.source.trim().replace('\\', "/");
    if !source.to_ascii_lowercase().contains(".ydd@") {
        return Err(format!(
            "static world prefab id='{}' source='{}' rejected: expected .ydd@entry",
            prefab.id, source
        ));
    }

    let authored_material = register_authored_prefab_material(mats, prefab);
    let root = spawn_named(world, format!("World/Static/{}", prefab.id));
    let _ = set_parent(world, root, Some(parent));
    let _ = world.insert(
        root,
        Transform {
            position: prefab.position,
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                prefab.rotation_ypr.x,
                prefab.rotation_ypr.y,
                prefab.rotation_ypr.z,
            ),
            scale: prefab.scale,
        },
    );
    crate::gameplay::attach_scene_object_core(
        world,
        root,
        prefab.position,
        Vec3::new(
            400.0 * prefab.scale.x.abs().max(0.001),
            100.0 * prefab.scale.y.abs().max(0.001),
            400.0 * prefab.scale.z.abs().max(0.001),
        ),
    );

    let mut part_count = 0u32;
    let mut triangle_count = 0u64;
    for (part_index, part) in decoded.iter().enumerate() {
        let primitive_id = part.primitive_id;
        let vertex_count = part.mesh.vertices.len();
        let index_count = part.mesh.indices.len();
        triangle_count = triangle_count.saturating_add((index_count / 3) as u64);
        if !prims.is_registered(primitive_id) {
            prims.register_mesh(primitive_id, part.name.clone(), part.mesh.clone());
        }
        let material_id =
            authored_material.unwrap_or_else(|| material_for_slot(materials, &part.material_slot));
        let mut render_options = mats
            .resolve(material_id)
            .map(|material| {
                if material.desc.flags.contains(MaterialFlags::ALPHA_TEST) {
                    newengine_model_domain_api::MeshRenderOptions::world_masked()
                } else {
                    newengine_model_domain_api::MeshRenderOptions::world_opaque()
                }
            })
            .unwrap_or_else(newengine_model_domain_api::MeshRenderOptions::world_opaque);
        if static_world_receive_only_shadow_slot(&part.material_slot) {
            render_options.shadow_policy =
                newengine_model_domain_api::MeshShadowPolicy::ReceiveOnly;
        }
        let entity = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id,
                material_id,
                name: &format!(
                    "World/Static/{}/{}-{part_index}",
                    prefab.id, part.material_slot
                ),
                position: Vec3::ZERO,
                scale: Vec3::ONE,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options,
            },
        );
        // Static imported geometry is currently visual-only. The procedural terrain
        // remains the authoritative walkable collision surface; this prevents a
        // single coarse collider from enclosing the entire winding road mesh.
        let _ = world.insert(
            entity,
            crate::gameplay::PhysicsSurface {
                id: if part.material_slot.to_ascii_lowercase().ends_with("_road") {
                    "surface.dirt_road"
                } else {
                    "surface.forest_ground"
                }
                .to_owned(),
                footstep_event: "audio.footstep.dirt".to_owned(),
                landing_event: "audio.landing.dirt".to_owned(),
            },
        );
        part_count = part_count.saturating_add(1);
        newengine_ulog_api::ulog::debug!(
            "static world part spawned prefab='{}' part='{}' vertices={} triangles={} material_id={:?}",
            prefab.id,
            part.material_slot,
            vertex_count,
            index_count / 3,
            material_id,
        );
    }

    Ok((part_count, triangle_count))
}

pub(super) fn begin_static_world_prefabs(
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
                        .eq_ignore_ascii_case(COLLISION_WORLD_PROXY))
        })
        .cloned()
        .collect::<Vec<_>>();
    // Keep visual/collision declarations for the same source adjacent so the
    // decoded mesh packet can be reused without another AssetManager/JSON pass.
    candidates.sort_by(|a, b| {
        let a_source = a.source.trim().replace('\\', "/");
        let b_source = b.source.trim().replace('\\', "/");
        a_source.cmp(&b_source).then_with(|| {
            let a_collision = a.proxy.trim().eq_ignore_ascii_case(COLLISION_WORLD_PROXY);
            let b_collision = b.proxy.trim().eq_ignore_ascii_case(COLLISION_WORLD_PROXY);
            a_collision.cmp(&b_collision)
        })
    });

    let total = candidates.len() as u32;
    world.insert_resource(GameReadyStaticWorldResidency {
        total,
        pending: total,
        ..GameReadyStaticWorldResidency::default()
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
    if let Some(residency) = world.resource_mut::<GameReadyStaticWorldResidency>() {
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
                .resource::<GameReadyStaticWorldResidency>()
                .map(GameReadyStaticWorldResidency::failed)
                .unwrap_or(0),
            elapsed_ms,
        );
        let _ = validate_scene_object_invariants(world, "game-ready.static-world-complete");
    } else {
        world.insert_resource(state);
    }
}
