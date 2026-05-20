
use core::f32::consts::TAU;

use newengine_assets::wait_ready;
use newengine_bounds::Bounds;
use newengine_core::{JobLane, JobPriority, JobRequest, JobSystemHandle, JobTicket};
use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, ShadowSettings};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Mat4, Quat, Vec3};
use newengine_primitives::{
    fnv1a_64, Primitive, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_procedural_noise::{
    DomainWarp2D, NoiseAlgorithm, NoiseCombineMode, NoiseDomain2D, NoiseGraph2D, NoiseLayer2D,
    NoiseRemap, NoiseShape, ProceduralTerrain, TerrainHeightfieldDescriptor,
};
use newengine_scene::{
    spawn_named, Scene, SceneCellCoord, SceneResidencySet, SceneStreamingBudget,
    SceneStreamingPlan,
};
use newengine_transform::{set_parent, Transform};

use std::sync::{Arc, Mutex};

use crate::gameplay::{
    spawn_player_controller_with_tuning, FpsDemoRules, FpsDemoState,
    FpsPlayerTuning, GameReadyWorldLaunchGate,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use self::content::{
    load_game_ready_map_profile, GameReadyFoliageSpec, GameReadyGameplaySpec,
    GameReadyLightingSpec, GameReadyMaterialSetSpec,
    GameReadyPaletteSpec, GameReadyPrefabSpec, GameReadySkySpec, GameReadyTerrainSpec,
};
use super::helpers::{
    apply_exact_material, apply_primitive_instance, ensure_primitive_base, ensure_root, primitive_bounds,
};

#[inline]
pub(super) fn game_ready_demo_enabled() -> bool {
    std::env::var("NEWENGINE_GAME_READY_DEMO")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
struct DemoMaterials {
    terrain: MaterialId,
    sky: MaterialId,
    tree_bark: MaterialId,
    tree_leaf: MaterialId,
    tree_branch: MaterialId,
}

#[derive(Clone, Copy)]
struct PrimitiveSpawnSpec<'a> {
    parent: EntityId,
    primitive_id: PrimitiveId,
    material_id: MaterialId,
    name: &'a str,
    position: Vec3,
    scale: Vec3,
    color: [f32; 4],
}

#[derive(Clone, Debug)]
pub(crate) struct TerrainSurfaceLayers {
    pub forest_base_texture: String,
    pub sand_base_texture: String,
    pub rock_base_texture: String,
    pub patch_scale: f32,
    pub blend_softness: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyDomeRuntime {
    pub follow_camera: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyCycleRuntime {
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
    pub base_sun_color: [f32; 3],
    pub base_sun_intensity: f32,
    pub base_ambient_color: [f32; 3],
    pub base_ambient_intensity: f32,
}

type TerrainChunkCoord = SceneCellCoord;

#[derive(Clone, Debug)]
struct TerrainChunkRecord {
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
struct GeneratedTerrainChunk {
    terrain: ProceduralTerrain,
    mesh: Arc<PrimitiveMesh>,
}

struct PendingTerrainChunk {
    result: Arc<Mutex<Option<GeneratedTerrainChunk>>>,
    ticket: JobTicket,
}

pub(crate) struct GameReadyTerrainStreamingState {
    root: EntityId,
    material: MaterialId,
    color: [f32; 4],
    spec: GameReadyTerrainSpec,
    surface: TerrainSurfaceLayers,
    chunk_radius: i32,
    unload_radius: i32,
    max_chunks_per_frame: usize,
    max_pending_jobs: usize,
    loaded: std::collections::BTreeMap<TerrainChunkCoord, TerrainChunkRecord>,
    pending: std::collections::BTreeMap<TerrainChunkCoord, PendingTerrainChunk>,
}



#[inline]
fn sky_smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-5)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn sky_lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
fn solar_direction_from_cycle(time_hours: f32, latitude_degrees: f32, axial_tilt_degrees: f32) -> Vec3 {
    let latitude = latitude_degrees.to_radians().clamp(-1.5533, 1.5533);
    let axial_tilt = axial_tilt_degrees.to_radians();
    let hour_angle = (time_hours / 24.0) * TAU - core::f32::consts::PI;
    let declination = axial_tilt * (hour_angle + core::f32::consts::FRAC_PI_2).sin();
    let altitude = (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos()).asin();
    let azimuth = hour_angle + core::f32::consts::PI;
    let horizon = altitude.cos().max(0.0);
    Vec3::new(azimuth.sin() * horizon, altitude.sin(), azimuth.cos() * horizon).normalize_or_zero()
}

pub fn tick_game_ready_sky_cycle(world: &mut newengine_ecs::World, dt: f32) {
    let (to_sun, sun_color, sun_intensity, ambient_color, ambient_intensity) = {
        let Some(cycle) = world.resource_mut::<SkyCycleRuntime>() else {
            return;
        };
        if !cycle.enabled {
            return;
        }

        let advance = if cycle.day_length_seconds > 0.0 {
            dt.max(0.0) * 24.0 / cycle.day_length_seconds
        } else {
            0.0
        };
        cycle.time_of_day_hours = (cycle.time_of_day_hours + advance).rem_euclid(24.0);

        let to_sun = solar_direction_from_cycle(
            cycle.time_of_day_hours,
            cycle.latitude_degrees,
            cycle.axial_tilt_degrees,
        );
        let elevation = to_sun.y;
        let day = sky_smoothstep(-0.08, 0.18, elevation);
        let horizon = 1.0 - sky_smoothstep(0.10, 0.55, elevation.abs());
        let warm = [1.0, 0.55, 0.27];
        let moon = [0.24, 0.30, 0.48];
        let noon = cycle.base_sun_color;
        let day_color = sky_lerp3(noon, warm, horizon * day);
        let sun_color = sky_lerp3(moon, day_color, day);
        let sun_intensity = cycle.base_sun_intensity * day.powf(1.18) + 0.035 * (1.0 - day);
        let ambient_color = sky_lerp3([0.025, 0.035, 0.075], cycle.base_ambient_color, day);
        let ambient_intensity = cycle.base_ambient_intensity * (0.10 + 0.90 * day) + 0.018 * (1.0 - day);

        (to_sun, sun_color, sun_intensity, ambient_color, ambient_intensity)
    };

    if let Some(ambient) = world.resource_mut::<AmbientLight>() {
        ambient.color = ambient_color;
        ambient.intensity = ambient_intensity;
    }

    let direction = -to_sun;
    let sun_entity = world.query::<DirectionalLight>().next().map(|(entity, _)| entity);
    if let Some(sun_entity) = sun_entity {
        if let Some(light) = world.get_mut_tracked::<DirectionalLight>(sun_entity) {
            light.direction_ws = [direction.x, direction.y, direction.z];
            light.color = sun_color;
            light.intensity = sun_intensity;
        }
    }
}

#[inline]
fn terrain_surface_layers(spec: &GameReadyTerrainSpec) -> TerrainSurfaceLayers {
    TerrainSurfaceLayers {
        forest_base_texture: spec.surface.forest_base_texture.clone(),
        sand_base_texture: spec.surface.sand_base_texture.clone(),
        rock_base_texture: spec.surface.rock_base_texture.clone(),
        patch_scale: spec.surface.patch_scale,
        blend_softness: spec.surface.blend_softness,
    }
}

#[inline]
fn spawn_game_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    spec: PrimitiveSpawnSpec<'_>,
) -> EntityId {
    let entity = spawn_named(world, spec.name);
    let _ = newengine_transform::set_parent(world, entity, Some(spec.parent));
    let _ = world.insert(entity, Primitive { id: spec.primitive_id, color: spec.color });

    if let Some(bounds) = primitive_bounds(prims, spec.primitive_id) {
        let _ = world.insert(entity, bounds);
    }

    ensure_primitive_base(world, entity, spec.material_id);
    apply_primitive_instance(world, mats, entity, spec.material_id, spec.color);

    if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
        t.position = spec.position;
        t.scale = spec.scale;
    }

    entity
}


#[inline]
fn register_demo_materials(
    mats: &MaterialRegistry,
    palette: &GameReadyPaletteSpec,
    materials: &GameReadyMaterialSetSpec,
) -> DemoMaterials {
    DemoMaterials {
        terrain: register_material(
            mats,
            "FPS/ProceduralTerrain",
            palette.terrain,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            &materials.terrain,
        ),
        sky: register_material(
            mats,
            "FPS/SkyDome",
            palette.sky,
            palette.sky_emissive,
            2.6,
            MaterialFlags::DOUBLE_SIDED,
            &materials.sky,
        ),
        tree_bark: register_material(
            mats,
            "FPS/Tree/Bark",
            palette.tree_bark,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            &materials.tree_bark,
        ),
        tree_leaf: register_material(
            mats,
            "FPS/Tree/Leaf",
            palette.tree_leaf,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::ALPHA_TEST)
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS),
            &materials.tree_leaf,
        ),
        tree_branch: register_material(
            mats,
            "FPS/Tree/Branch",
            palette.tree_branch,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS),
            &materials.tree_branch,
        ),
    }
}

#[inline]
fn configure_game_ready_lighting(world: &mut newengine_ecs::World, spec: &GameReadyLightingSpec) {
    let ambient = AmbientLight {
        color: spec.ambient_color,
        intensity: spec.ambient_intensity,
    };
    match world.resource_mut::<AmbientLight>() {
        Some(a) => *a = ambient,
        None => world.insert_resource(ambient),
    }

    let sun_dir = Vec3::new(
        spec.sun_direction[0],
        spec.sun_direction[1],
        spec.sun_direction[2],
    )
    .normalize_or_zero();
    let sun = DirectionalLight {
        direction_ws: [sun_dir.x, sun_dir.y, sun_dir.z],
        color: spec.sun_color,
        intensity: spec.sun_intensity,
    };
    let sun_entity = world.query::<DirectionalLight>().next().map(|(entity, _)| entity);
    if let Some(sun_entity) = sun_entity {
        if let Some(light) = world.get_mut_tracked::<DirectionalLight>(sun_entity) {
            *light = sun;
        }
    } else {
        let sun_entity = spawn_named(world, "Game/Sun");
        let _ = world.insert(sun_entity, sun);
    }

    world.insert_resource(SkyCycleRuntime {
        enabled: spec.day_night.enabled,
        time_of_day_hours: spec.day_night.time_of_day_hours,
        day_length_seconds: spec.day_night.day_length_seconds,
        latitude_degrees: spec.day_night.latitude_degrees,
        axial_tilt_degrees: spec.day_night.axial_tilt_degrees,
        base_sun_color: spec.sun_color,
        base_sun_intensity: spec.sun_intensity,
        base_ambient_color: spec.ambient_color,
        base_ambient_intensity: spec.ambient_intensity,
    });
    tick_game_ready_sky_cycle(world, 0.0);

    log::info!(
        "game-ready sky cycle: tod={:.2}h day_len={:.1}s ambient={:?}/{:.3} sun_dir={:?} sun={:?}/{:.3} shadows={} strength={:.3}",
        spec.day_night.time_of_day_hours,
        spec.day_night.day_length_seconds,
        ambient.color,
        ambient.intensity,
        sun.direction_ws,
        sun.color,
        sun.intensity,
        spec.shadows.enabled,
        spec.shadows.contact_strength,
    );

    world.insert_resource(ShadowSettings {
        enabled: spec.shadows.enabled,
        method: newengine_lighting::ShadowMethod::DirectionalDepthMap,
        resolution: spec.shadows.resolution,
        cascade_count: spec.shadows.cascade_count,
        max_distance: spec.shadows.max_distance,
        softness: spec.shadows.softness,
        bias: spec.shadows.bias,
        normal_bias: spec.shadows.normal_bias,
        contact_strength: spec.shadows.contact_strength,
    });
}


fn terrain_graph_for_chunk(spec: &GameReadyTerrainSpec, coord: TerrainChunkCoord) -> NoiseGraph2D {
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
            .shape(NoiseShape::SmoothStep { edge0: -0.72, edge1: 0.42 })
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
            .shape(NoiseShape::SmoothStep { edge0: 0.12, edge1: 0.95 })
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

fn generate_terrain_for_chunk(spec: &GameReadyTerrainSpec, coord: TerrainChunkCoord, color: [f32; 4]) -> GeneratedTerrainChunk {
    let terrain = ProceduralTerrain::generate_descriptor(
        TerrainHeightfieldDescriptor {
            cells_x: spec.cells_x,
            cells_z: spec.cells_z,
            size_x: spec.size_x,
            size_z: spec.size_z,
            base_height: spec.base_height,
            height_scale: spec.height_scale,
            graph: terrain_graph_for_chunk(spec, coord),
            smoothing_passes: spec.generator.smoothing_passes,
            smoothing_strength: spec.generator.smoothing_strength,
        },
        color,
    );
    // Build the renderable primitive mesh on the generation lane as well.
    // Previously every committed streamed chunk did this conversion inside the
    // render draw-list extraction path; in debug/profile-dev this cost dominated
    // the frame and made the FPS overlay report ~3 FPS while the Vulkan backend
    // itself was idle.
    let mesh = Arc::new(terrain.heightfield.to_primitive_mesh());
    GeneratedTerrainChunk { terrain, mesh }
}

fn spawn_generated_terrain_chunk(
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
    let _ = world.insert(entity, PreparedTerrainPrimitiveMesh { mesh: generated.mesh });
    let _ = world.insert(entity, bounds);
    let _ = world.insert(entity, surface.clone());
    let _ = apply_exact_material(world, mats, entity, material, material, color);

    TerrainChunkRecord { terrain: entity }
}

fn spawn_streamed_terrain_chunk(
    world: &mut newengine_ecs::World,
    root: EntityId,
    mats: &MaterialRegistry,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    surface: &TerrainSurfaceLayers,
    color: [f32; 4],
    coord: TerrainChunkCoord,
) -> TerrainChunkRecord {
    let generated = generate_terrain_for_chunk(spec, coord, color);
    spawn_generated_terrain_chunk(world, root, mats, material, spec, surface, color, coord, generated)
}

fn enqueue_streamed_terrain_chunk(
    state: &mut GameReadyTerrainStreamingState,
    job_system: Option<&JobSystemHandle>,
    coord: TerrainChunkCoord,
) -> bool {
    if state.pending.contains_key(&coord) || state.loaded.contains_key(&coord) {
        return false;
    }
    if state.pending.len() >= state.max_pending_jobs.max(1) {
        return false;
    }

    let Some(job_system) = job_system else {
        return false;
    };

    let spec = state.spec.clone();
    let color = state.color;
    let result = Arc::new(Mutex::new(None));
    let result_for_job = Arc::clone(&result);
    let ticket = job_system.submit_request(
        JobRequest::new("game-ready.terrain.chunk.generate")
            .with_lane(JobLane::Streaming)
            .with_priority(JobPriority::Normal),
        move || {
            let generated = generate_terrain_for_chunk(&spec, coord, color);
            if let Ok(mut slot) = result_for_job.lock() {
                *slot = Some(generated);
            }
        },
    );
    state.pending.insert(coord, PendingTerrainChunk { result, ticket });
    true
}

fn spawn_procedural_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    root: EntityId,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    color: [f32; 4],
    initial_center: TerrainChunkCoord,
) -> EntityId {
    log::info!(
        "game-ready: terrain generator id='{}' seed={} cells={}x{} chunk_size={}x{} streaming={} radius={} unload_radius={} surface_layers=[forest='{}', sand='{}', rock='{}']",
        spec.generator.id,
        spec.seed,
        spec.cells_x,
        spec.cells_z,
        spec.size_x,
        spec.size_z,
        spec.streaming.enabled,
        spec.streaming.chunk_radius,
        spec.streaming.unload_radius,
        spec.surface.forest_base_texture,
        spec.surface.sand_base_texture,
        spec.surface.rock_base_texture,
    );

    let surface = terrain_surface_layers(spec);
    let origin = initial_center;
    let record = spawn_streamed_terrain_chunk(world, root, mats, material, spec, &surface, color, origin);
    let terrain_entity = record.terrain;

    if spec.streaming.enabled {
        let budget = SceneStreamingBudget {
            resident_radius: spec.streaming.chunk_radius,
            unload_radius: spec.streaming.unload_radius,
            max_commits_per_tick: spec.streaming.max_chunks_per_frame,
        }
        .sanitized();
        let mut state = GameReadyTerrainStreamingState {
            root,
            material,
            color,
            spec: spec.clone(),
            surface,
            chunk_radius: budget.resident_radius,
            unload_radius: budget.unload_radius,
            max_chunks_per_frame: budget.max_commits_per_tick,
            max_pending_jobs: budget.max_commits_per_tick.saturating_mul(4).max(4),
            loaded: std::collections::BTreeMap::new(),
            pending: std::collections::BTreeMap::new(),
        };
        state.loaded.insert(origin, record);

        // The initial resident ring must be present before the public launch gate
        // opens. Otherwise the first playable frames stream the remaining chunks
        // one by one, and each cold terrain GPU upload lands on the render
        // extraction path. In the current GameReady profile that produced visible
        // ~250-300 ms frame gaps while Vulkan submit itself stayed mostly idle.
        let mut warmed = 1usize;
        for coord in SceneResidencySet::desired_cells(origin, state.chunk_radius) {
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
            );
            state.loaded.insert(coord, record);
            warmed = warmed.saturating_add(1);
        }
        if warmed > 1 {
            log::info!(
                "game-ready terrain streaming: initial resident chunks warmed center=[{},{}] radius={} chunks={}",
                origin.x,
                origin.z,
                state.chunk_radius,
                warmed
            );
        }

        world.insert_resource(state);
    }

    terrain_entity
}

pub(crate) fn tick_game_ready_streaming_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    job_system: Option<&JobSystemHandle>,
) {
    let Some(player) = crate::gameplay::first_player(world) else {
        return;
    };
    let player_pos = world
        .get::<Transform>(player)
        .map(|t| t.position)
        .unwrap_or(Vec3::ZERO);

    let Some(mut state) = world.remove_resource::<GameReadyTerrainStreamingState>() else {
        return;
    };

    let center = TerrainChunkCoord::from_world_pos(player_pos, state.spec.size_x, state.spec.size_z);
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

    let plan = SceneStreamingPlan::build(
        center,
        budget,
        state.loaded.keys().copied(),
        state.pending.keys().copied(),
    );

    let commit_budget = budget.max_commits_per_tick.max(1);
    let mut created = 0usize;
    let completed = state
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
        .take(commit_budget)
        .collect::<Vec<_>>();
    for coord in completed {
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
    let mut scheduled = 0usize;
    for request in plan.loads.iter().take(remaining_commit_budget) {
        let coord = request.coord;
        if state.loaded.contains_key(&coord) || state.pending.contains_key(&coord) {
            continue;
        }

        if enqueue_streamed_terrain_chunk(&mut state, job_system, coord) {
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

    if created > 0 || scheduled > 0 || removed > 0 {
        log::debug!(
            "game-ready terrain streaming: center=[{},{}] loaded={} pending={} created={} scheduled={} removed={} planned_loads={} planned_unloads={}",
            center.x,
            center.z,
            state.loaded.len(),
            state.pending.len(),
            created,
            scheduled,
            removed,
            plan.loads.len(),
            plan.unloads.len(),
        );
    }

    world.insert_resource(state);
}
