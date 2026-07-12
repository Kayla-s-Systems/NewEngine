use super::terrain_heightmap::{load_terrain_heightmap, TerrainHeightmapRuntime};
use super::*;
use std::time::{Duration, Instant};

// Terrain streaming owns chunk residency, procedural heightfield generation
// and precomputed render mesh payloads. Material registration and sky
// lifecycle stay in their own canonical modules.

#[derive(Clone, Debug)]
pub(crate) struct TerrainSurfaceLayers {
    pub forest_base_texture: String,
    pub sand_base_texture: String,
    pub rock_base_texture: String,
    pub patch_scale: f32,
    pub blend_softness: f32,
}

type TerrainChunkCoord = SceneCellCoord;

#[derive(Clone, Debug)]
pub(super) struct TerrainChunkRecord {
    terrain: EntityId,
}

/// CPU-prepared terrain mesh payload for render upload.
///
/// This is intentionally an engine-runtime scene component, not a render-provider
/// type. Terrain generation jobs can build the expensive heightfield-to-mesh
/// conversion off the frame thread, while the renderer still receives only the
/// normal procedural-terrain ECS data and uploads through `engine.render`.
#[derive(Clone, Debug)]
pub(crate) struct PreparedTerrainPrimitiveMesh {
    pub mesh: Arc<PrimitiveMesh>,
}

#[derive(Clone, Debug)]
pub(super) struct GeneratedTerrainChunk {
    terrain: ProceduralTerrain,
    mesh: Arc<PrimitiveMesh>,
}

pub(super) struct PendingTerrainChunk {
    result: Arc<Mutex<Option<GeneratedTerrainChunk>>>,
    ticket: TaskTicket,
}

pub(crate) struct GameReadyTerrainStreamingState {
    root: EntityId,
    anchor: EntityId,
    material: MaterialId,
    color: [f32; 4],
    spec: GameReadyTerrainSpec,
    surface: TerrainSurfaceLayers,
    heightmap: Option<Arc<TerrainHeightmapRuntime>>,
    chunk_radius: i32,
    unload_radius: i32,
    max_chunks_per_frame: usize,
    max_pending_jobs: usize,
    stream_commit_count: u64,
    last_stream_commit_at: Option<Instant>,
    loaded: std::collections::BTreeMap<TerrainChunkCoord, TerrainChunkRecord>,
    pending: std::collections::BTreeMap<TerrainChunkCoord, PendingTerrainChunk>,
}

#[inline]
pub(super) fn terrain_surface_layers(spec: &GameReadyTerrainSpec) -> TerrainSurfaceLayers {
    TerrainSurfaceLayers {
        forest_base_texture: spec.surface.forest_base_texture.clone(),
        sand_base_texture: spec.surface.sand_base_texture.clone(),
        rock_base_texture: spec.surface.rock_base_texture.clone(),
        patch_scale: spec.surface.patch_scale,
        blend_softness: spec.surface.blend_softness,
    }
}

#[inline]
fn launch_blocking_warm_radius(target_radius: i32) -> i32 {
    const DEFAULT_LAUNCH_WARM_RADIUS: i32 = 1;
    let requested = std::env::var("NEWENGINE_SCENE_TERRAIN_LAUNCH_WARM_RADIUS")
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(DEFAULT_LAUNCH_WARM_RADIUS);
    requested.clamp(0, target_radius.max(0))
}

#[inline]
fn responsive_stream_commit_budget(
    target_budget: usize,
    state: &mut GameReadyTerrainStreamingState,
) -> usize {
    if target_budget == 0 {
        return 0;
    }

    let burst = std::env::var("NEWENGINE_SCENE_TERRAIN_STREAM_BURST")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, target_budget.max(1));
    let interval_ms = std::env::var("NEWENGINE_SCENE_TERRAIN_STREAM_INTERVAL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(140)
        .clamp(16, 2_000);

    let now = Instant::now();
    if let Some(last) = state.last_stream_commit_at {
        if now.duration_since(last) < Duration::from_millis(interval_ms) {
            return 0;
        }
    }

    state.last_stream_commit_at = Some(now);
    state.stream_commit_count = state.stream_commit_count.saturating_add(1);
    burst
}

pub(super) fn terrain_graph_for_chunk(
    spec: &GameReadyTerrainSpec,
    coord: TerrainChunkCoord,
) -> NoiseGraph2D {
    let center = coord.center(spec.size_x, spec.size_z);

    // GameFirst terrain is intentionally not a mountain generator. The profile
    // produces traversable land: shallow depressions, low ridges, dry creek-like
    // cuts, and broad biome patches. Vertical relief stays modest; visual
    // diversity comes from surface masks, foliage density, and local terrain
    // character rather than endless hills.
    let mut terrain_graph = NoiseGraph2D::new(NoiseDomain2D {
        seed: spec.seed,
        frequency: 0.018,
        offset_x: 0.0,
        offset_z: 0.0,
        warp: Some(DomainWarp2D {
            seed_offset: 0x91e7_70ad,
            frequency: 0.045,
            strength: 3.8,
            octaves: 3,
        }),
    })
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Value)
            .combine(NoiseCombineMode::Replace)
            .frequency(1.0)
            .amplitude(0.34),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Cellular)
            .seed_offset(spec.seed ^ 0x6c8e_9cf5)
            .frequency(0.42)
            .amplitude(0.18)
            .shape(NoiseShape::SmoothStep {
                edge0: -0.72,
                edge1: 0.42,
            })
            .combine(NoiseCombineMode::Add),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Billow)
            .seed_offset(spec.seed ^ 0x2f4d_31aa)
            .frequency(2.75)
            .amplitude(0.08)
            .combine(NoiseCombineMode::Add),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Ridged)
            .seed_offset(spec.seed ^ spec.generator.ridged_seed_xor)
            .frequency(spec.generator.ridged_frequency)
            .amplitude(spec.generator.ridged_amplitude)
            .shape(NoiseShape::SmoothStep {
                edge0: spec.generator.ridged_shape_edge0,
                edge1: spec.generator.ridged_shape_edge1,
            })
            .combine(NoiseCombineMode::Add),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Veins)
            .seed_offset(spec.seed ^ spec.generator.veins_seed_xor)
            .frequency(spec.generator.veins_frequency)
            .amplitude(-spec.generator.veins_amplitude.abs())
            .shape(NoiseShape::SmoothStep {
                edge0: 0.12,
                edge1: 0.95,
            })
            .combine(NoiseCombineMode::Add),
    )
    .with_remap(NoiseRemap {
        input_min: -0.55,
        input_max: 0.65,
        output_min: -0.45,
        output_max: 0.68,
        clamp: true,
    });

    terrain_graph.domain.offset_x += center.x * terrain_graph.domain.frequency;
    terrain_graph.domain.offset_z += center.z * terrain_graph.domain.frequency;
    terrain_graph
}

pub(super) fn generate_terrain_for_chunk(
    spec: &GameReadyTerrainSpec,
    coord: TerrainChunkCoord,
    color: [f32; 4],
    heightmap: Option<&TerrainHeightmapRuntime>,
) -> GeneratedTerrainChunk {
    let center = coord.center(spec.size_x, spec.size_z);
    let descriptor = TerrainHeightfieldDescriptor {
        cells_x: spec.cells_x,
        cells_z: spec.cells_z,
        size_x: spec.size_x,
        size_z: spec.size_z,
        base_height: spec.base_height,
        height_scale: spec.height_scale,
        graph: terrain_graph_for_chunk(spec, coord),
        smoothing_passes: spec.generator.smoothing_passes,
        smoothing_strength: spec.generator.smoothing_strength,
    };
    let terrain = if let Some(heightmap) = heightmap {
        ProceduralTerrain::generate_descriptor_with_world_height_modifier(
            descriptor,
            color,
            heightmap.revision_key(),
            |local_x, local_z, procedural_height| {
                heightmap.apply_world_height(
                    center.x + local_x,
                    center.z + local_z,
                    procedural_height,
                )
            },
        )
    } else {
        ProceduralTerrain::generate_descriptor(descriptor, color)
    };
    // Build the renderable primitive mesh on the generation lane as well.
    // Previously every committed streamed chunk did this conversion inside the
    // render draw-list extraction path; in debug/profile-dev this cost dominated
    // the frame and made the FPS overlay report ~3 FPS while the Vulkan backend
    // itself was idle.
    let mesh = Arc::new(terrain.heightfield.to_primitive_mesh());
    GeneratedTerrainChunk { terrain, mesh }
}

pub(super) fn spawn_generated_terrain_chunk(
    world: &mut newengine_ecs::World,
    root: EntityId,
    mats: &MaterialRegistry,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    surface: &TerrainSurfaceLayers,
    color: [f32; 4],
    coord: TerrainChunkCoord,
    generated: GeneratedTerrainChunk,
) -> TerrainChunkRecord {
    let center = coord.center(spec.size_x, spec.size_z);
    let terrain = generated.terrain;
    let bounds = Bounds::from_local_aabb(terrain.heightfield.local_bounds());
    let entity = spawn_named(world, format!("Terrain/Chunk[{:+},{:+}]", coord.x, coord.z));
    let _ = newengine_transform::set_parent(world, entity, Some(root));
    let _ = world.insert(
        entity,
        Transform {
            position: center,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(entity, terrain);
    let _ = world.insert(
        entity,
        PreparedTerrainPrimitiveMesh {
            mesh: generated.mesh,
        },
    );
    let terrain_half_extents = bounds.local_aabb.half_extents();
    let _ = world.insert(entity, bounds);
    crate::gameplay::attach_scene_object_core(world, entity, center, terrain_half_extents);
    let _ = world.insert(entity, surface.clone());
    let _ = apply_exact_material(world, mats, entity, material, material, color);

    TerrainChunkRecord { terrain: entity }
}

pub(super) fn spawn_streamed_terrain_chunk(
    world: &mut newengine_ecs::World,
    root: EntityId,
    mats: &MaterialRegistry,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    surface: &TerrainSurfaceLayers,
    color: [f32; 4],
    coord: TerrainChunkCoord,
    heightmap: Option<&TerrainHeightmapRuntime>,
) -> TerrainChunkRecord {
    let generated = generate_terrain_for_chunk(spec, coord, color, heightmap);
    spawn_generated_terrain_chunk(
        world, root, mats, material, spec, surface, color, coord, generated,
    )
}

pub(super) fn enqueue_streamed_terrain_chunk(
    state: &mut GameReadyTerrainStreamingState,
    thread_pool: Option<&ThreadPoolHandle>,
    coord: TerrainChunkCoord,
) -> bool {
    if state.pending.contains_key(&coord) || state.loaded.contains_key(&coord) {
        return false;
    }
    if state.pending.len() >= state.max_pending_jobs.max(1) {
        return false;
    }

    let Some(thread_pool) = thread_pool else {
        return false;
    };

    let spec = state.spec.clone();
    let color = state.color;
    let heightmap = state.heightmap.clone();
    let result = Arc::new(Mutex::new(None));
    let result_for_job = Arc::clone(&result);
    let ticket = thread_pool.submit_request(
        TaskRequest::new("game-ready.terrain.chunk.render-packet")
            .with_source("scene.streaming.terrain")
            .with_owner("engine.render")
            .with_category("terrain.render-packet")
            .with_lane(TaskLane::RenderPrep)
            .with_priority(TaskPriority::Interactive)
            .with_dependency_group(format!("terrain.chunk.{}.{}.renderprep", coord.x, coord.z))
            .with_task_domain(task_domain::ENGINE_RENDER_PREP)
            .with_task_pass(task_pass::TERRAIN_RENDER_PACKET),
        move || {
            let generated = generate_terrain_for_chunk(&spec, coord, color, heightmap.as_deref());
            if let Ok(mut slot) = result_for_job.lock() {
                *slot = Some(generated);
            }
        },
    );
    state
        .pending
        .insert(coord, PendingTerrainChunk { result, ticket });
    true
}

pub(in crate::scene_bridge::game_ready) fn spawn_procedural_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    root: EntityId,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    color: [f32; 4],
    initial_center: TerrainChunkCoord,
) -> EntityId {
    newengine_ulog_api::ulog::info!(
        "game-ready: terrain generator id='{}' seed={} cells={}x{} chunk_size={}x{} streaming={} radius={} unload_radius={} surface_mode='multi_textured' layer_count={} heightmap_enabled={} surface_layers=[forest='{}', sand='{}', rock='{}']",
        spec.generator.id,
        spec.seed,
        spec.cells_x,
        spec.cells_z,
        spec.size_x,
        spec.size_z,
        spec.streaming.enabled,
        spec.streaming.chunk_radius,
        spec.streaming.unload_radius,
        spec.surface.layers.len(),
        spec.heightmap.enabled,
        spec.surface.forest_base_texture,
        spec.surface.sand_base_texture,
        spec.surface.rock_base_texture,
    );

    if !spec.surface.layers.is_empty() {
        let summary = spec
            .surface
            .layers
            .iter()
            .map(|layer| {
                format!(
                    "{}:texture='{}':weight={:.3}:uv_scale={:.3}",
                    layer.role, layer.base_texture, layer.weight, layer.uv_scale
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        newengine_ulog_api::ulog::info!(
            "game-ready terrain surface package: declarative_layers={} projection='3-channel terrain shader' layers=[{}]",
            spec.surface.layers.len(),
            summary
        );
    }

    let surface = terrain_surface_layers(spec);
    let heightmap = load_terrain_heightmap(spec);
    let streaming_anchor = spawn_named(world, "Scene/Terrain/StreamingAnchor");
    let _ = set_parent(world, streaming_anchor, Some(root));
    crate::gameplay::attach_scene_element_core(
        world,
        streaming_anchor,
        crate::gameplay::SceneEntityRole::TerrainStreamingAnchor,
        "Scene/Terrain/StreamingAnchor",
        Vec3::ZERO,
        Vec3::splat(1.0),
    );
    let _ = world.insert(
        streaming_anchor,
        crate::gameplay::SceneAnchorFollow::player(),
    );

    let origin = initial_center;
    let record = spawn_streamed_terrain_chunk(
        world,
        root,
        mats,
        material,
        spec,
        &surface,
        color,
        origin,
        heightmap.as_deref(),
    );
    let terrain_entity = record.terrain;
    newengine_ulog_api::ulog::info!(
        "game-ready terrain anchor: terrain_entity={:?} streaming_anchor={:?} parent={:?} policy='terrain streaming target is an ordinary ECS entity anchor'",
        terrain_entity,
        streaming_anchor,
        root
    );

    if spec.streaming.enabled {
        let budget = SceneStreamingBudget {
            resident_radius: spec.streaming.chunk_radius,
            unload_radius: spec.streaming.unload_radius,
            max_commits_per_tick: spec.streaming.max_chunks_per_frame,
        }
        .sanitized();
        let mut state = GameReadyTerrainStreamingState {
            root,
            anchor: streaming_anchor,
            material,
            color,
            spec: spec.clone(),
            surface,
            heightmap: heightmap.clone(),
            chunk_radius: budget.resident_radius,
            unload_radius: budget.unload_radius,
            max_chunks_per_frame: budget.max_commits_per_tick,
            max_pending_jobs: budget.max_commits_per_tick.saturating_mul(4).max(4),
            stream_commit_count: 0,
            last_stream_commit_at: None,
            loaded: std::collections::BTreeMap::new(),
            pending: std::collections::BTreeMap::new(),
        };
        state.loaded.insert(origin, record);

        // Keep the native window responsive during first world handoff. The full
        // streaming target radius remains active, but only a small launch ring is
        // generated synchronously before the public launch gate opens. Remaining
        // chunks are admitted by tick_game_ready_streaming_terrain() through the
        // normal frame-budgeted streaming path.
        let launch_warm_radius = launch_blocking_warm_radius(state.chunk_radius);
        let mut warmed = 1usize;
        for coord in SceneResidencySet::desired_cells(origin, launch_warm_radius) {
            if state.loaded.contains_key(&coord) {
                continue;
            }
            let record = spawn_streamed_terrain_chunk(
                world,
                state.root,
                mats,
                state.material,
                &state.spec,
                &state.surface,
                state.color,
                coord,
                state.heightmap.as_deref(),
            );
            state.loaded.insert(coord, record);
            warmed = warmed.saturating_add(1);
        }
        if warmed > 1 || launch_warm_radius != state.chunk_radius {
            let target_chunks = SceneResidencySet::desired_cells(origin, state.chunk_radius).len();
            newengine_ulog_api::ulog::info!(
                "game-ready terrain streaming: initial resident chunks warmed center=[{},{}] launch_radius={} target_radius={} chunks={} target_chunks={} policy='responsive startup; remaining chunks stream after launch'",
                origin.x,
                origin.z,
                launch_warm_radius,
                state.chunk_radius,
                warmed,
                target_chunks
            );
        }

        world.insert_resource(state);
    }

    terrain_entity
}

pub(crate) fn tick_game_ready_streaming_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    let Some(player) = crate::gameplay::first_player(world) else {
        return;
    };
    let player_pos = world
        .get::<Transform>(player)
        .map(|t| t.position)
        .unwrap_or(Vec3::ZERO);
    let role_anchor = crate::gameplay::scene_entity_by_role(
        world,
        crate::gameplay::SceneEntityRole::TerrainStreamingAnchor,
    );

    let Some(mut state) = world.remove_resource::<GameReadyTerrainStreamingState>() else {
        return;
    };
    state.anchor = role_anchor.unwrap_or(state.anchor);
    let follow_player = world
        .get::<crate::gameplay::SceneAnchorFollow>(state.anchor)
        .map(|follow| follow.enabled)
        .unwrap_or(false);
    if follow_player {
        if let Some(t) = world.get_mut_tracked::<Transform>(state.anchor) {
            t.position = player_pos;
        }
    }
    let anchor_pos = world
        .get::<Transform>(state.anchor)
        .map(|t| t.position)
        .unwrap_or(player_pos);

    let center =
        TerrainChunkCoord::from_world_pos(anchor_pos, state.spec.size_x, state.spec.size_z);
    let budget = SceneStreamingBudget {
        resident_radius: state.chunk_radius,
        unload_radius: state.unload_radius,
        max_commits_per_tick: state.max_chunks_per_frame,
    }
    .sanitized();
    state.chunk_radius = budget.resident_radius;
    state.unload_radius = budget.unload_radius;
    state.max_chunks_per_frame = budget.max_commits_per_tick;
    state.max_pending_jobs = state
        .max_pending_jobs
        .max(budget.max_commits_per_tick.saturating_mul(4).max(4));

    let profile = SceneStreamingProfile {
        render: budget,
        simulation: SceneStreamingBudget {
            resident_radius: budget.resident_radius.saturating_add(2),
            unload_radius: budget.unload_radius.saturating_add(2),
            // Coarse simulation is intentionally cheaper than render residency.
            max_commits_per_tick: budget.max_commits_per_tick.saturating_div(2).max(1),
        },
    }
    .sanitized();
    let layered_plan = SceneLayeredStreamingPlan::build(
        center,
        profile,
        state.loaded.keys().copied(),
        state.pending.keys().copied(),
        std::iter::empty::<TerrainChunkCoord>(),
        std::iter::empty::<TerrainChunkCoord>(),
    );
    let plan = &layered_plan.render;
    let bucket_plan = SceneBucketedCellPlan::from_desired_sets(
        center,
        layered_plan.render.desired.iter().copied(),
        layered_plan.simulation.desired.iter().copied(),
    );

    let loaded_coords = state
        .loaded
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let pending_coords = state
        .pending
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let completed_ready = state
        .pending
        .keys()
        .copied()
        .filter(|coord| {
            state
                .pending
                .get(coord)
                .map(|pending| pending.ticket.is_complete())
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let stream_requests_ready = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_render_residency())
        .map(|cell| cell.coord)
        .filter(|coord| !loaded_coords.contains(coord) && !pending_coords.contains(coord))
        .collect::<Vec<_>>();

    let target_commit_budget = budget.max_commits_per_tick.max(1);
    let commit_budget = if completed_ready.is_empty() && stream_requests_ready.is_empty() {
        0
    } else {
        responsive_stream_commit_budget(target_commit_budget, &mut state)
    };
    let mut created = 0usize;
    for coord in completed_ready.into_iter().take(commit_budget) {
        let Some(pending) = state.pending.remove(&coord) else {
            continue;
        };
        let generated = pending.result.lock().ok().and_then(|mut slot| slot.take());
        if let Some(generated) = generated {
            let record = spawn_generated_terrain_chunk(
                world,
                state.root,
                mats,
                state.material,
                &state.spec,
                &state.surface,
                state.color,
                coord,
                generated,
            );
            state.loaded.insert(coord, record);
            created += 1;
        }
    }

    let remaining_commit_budget = commit_budget.saturating_sub(created);
    let stream_requests = stream_requests_ready
        .into_iter()
        .take(remaining_commit_budget)
        .collect::<Vec<_>>();

    let mut scheduled = 0usize;
    for coord in stream_requests {
        if enqueue_streamed_terrain_chunk(&mut state, thread_pool, coord) {
            scheduled += 1;
            continue;
        }

        let record = spawn_streamed_terrain_chunk(
            world,
            state.root,
            mats,
            state.material,
            &state.spec,
            &state.surface,
            state.color,
            coord,
            state.heightmap.as_deref(),
        );
        state.loaded.insert(coord, record);
        created += 1;
        scheduled += 1;
    }

    let mut removed = 0usize;
    for request in &plan.unloads {
        let coord = request.coord;
        if let Some(record) = state.loaded.remove(&coord) {
            let _ = world.despawn(record.terrain);
            removed += 1;
        }
    }

    let to_drop_pending = state
        .pending
        .keys()
        .copied()
        .filter(|coord| coord.chebyshev_distance(center) > budget.unload_radius)
        .collect::<Vec<_>>();
    for coord in to_drop_pending {
        state.pending.remove(&coord);
    }

    let streaming_changed = created > 0 || scheduled > 0 || removed > 0;
    let render_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_render_residency())
        .count();
    let simulation_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_simulation_residency())
        .count();
    let reached_render_target = state.loaded.len() >= render_desired && state.pending.is_empty();
    let diagnostics_due =
        removed > 0 || state.stream_commit_count.is_multiple_of(16) || reached_render_target;

    if streaming_changed && diagnostics_due {
        newengine_ulog_api::ulog::debug!(
            "game-ready terrain streaming: center=[{},{}] anchor={:?} follow_player={} render_loaded={} render_pending={} created={} scheduled={} removed={} commit_budget={} commit_count={} render_desired={} render_unloads={} sim_desired={}",
            center.x,
            center.z,
            state.anchor,
            follow_player,
            state.loaded.len(),
            state.pending.len(),
            created,
            scheduled,
            removed,
            commit_budget,
            state.stream_commit_count,
            render_desired,
            plan.unloads.len(),
            simulation_desired,
        );
        let _ = validate_scene_object_invariants(world, "game-ready.streaming-terrain");
    }

    world.insert_resource(state);
}
